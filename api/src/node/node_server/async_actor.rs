use std::sync::Arc;

use actix::{
    ActorFutureExt, AsyncContext, Handler, Message, Recipient, ResponseActFuture, WrapFuture,
};

use crate::{
    audio_hosts::youtube::{
        playlist::get_playlist_video_urls, youtube_content_type, YoutubeContentType,
    },
    audio_playback::audio_item::{AudioMetadata, AudioPlayerQueueItem},
    commands::node_commands::{AddQueueItemParams, AudioIdentifier},
    database::{
        fetch_data::{get_audio_metadata_from_db, get_playlist_items_from_db},
        store_data::{store_playlist_if_not_exists, store_playlist_item_relation_if_not_exists},
    },
    downloader::{
        actor::{DownloadAudioRequest, NotifyDownloadUpdate},
        download_identifier::{
            AudioKind, Identifier, ItemUid, YoutubePlaylistUrl, YoutubeVideoUrl,
        },
        DownloadRequiredInformation, YoutubePlaylistDownloadInfo,
    },
    error::{AppError, AppErrorKind, IntoAppError},
    node::node_server::extract_queue_metadata,
    streams::node_streams::AudioNodeInfoStreamMessage,
    utils::log_msg_received,
    yt_api_key,
};

use super::{clean_url, AudioNode, AudioUrl};

#[derive(Debug, Clone, Message)]
#[rtype(result = "()")]
pub struct AsyncAddQueueItem(pub AddQueueItemParams);

#[derive(Debug)]
pub enum LocalAudioMetadata {
    Found {
        metadata: AudioMetadata,
        uid: ItemUid<Arc<str>>,
    },
    NotFound {
        url: AudioUrl,
    },
}

#[derive(Debug, Message)]
#[rtype(result = "()")]
struct LocalAudioMetadataList {
    list_url: AudioUrl,
    metadata: Vec<LocalAudioMetadata>,
}

impl Handler<AsyncAddQueueItem> for AudioNode {
    type Result = ResponseActFuture<Self, ()>;

    fn handle(&mut self, msg: AsyncAddQueueItem, _ctx: &mut Self::Context) -> Self::Result {
        log_msg_received(&self, &msg);

        enum MetadataQueryResult {
            Single(LocalAudioMetadata),
            Many(LocalAudioMetadataList),
            ManyLocal(Arc<[(ItemUid<Arc<str>>, AudioMetadata)]>),
        }

        Box::pin(
            async move {
                let identifier = match msg.0.identifier.into_required_info().await {
                    Ok(ident) => ident,
                    Err(err) => {
                        return Err(err);
                    }
                };

                let query_res: Result<MetadataQueryResult, AppError> = match identifier {
                    DownloadRequiredInformation::StoredLocally { uid } => {
                        let uid = ItemUid(uid);
                        let kind = AudioKind::from_uid(&uid);

                        match kind {
                            Some(AudioKind::YoutubeVideo) => {
                                match get_audio_metadata_from_db(&uid).await {
                                    Ok(Some(metadata)) => {
                                        Ok(MetadataQueryResult::Single(LocalAudioMetadata::Found {
                                            metadata,
                                            uid,
                                        }))
                                    }
                                    Ok(None) => Err(AppError::new(
                                        AppErrorKind::LocalData,
                                        "failed to find audio data locally",
                                        &[],
                                    )),
                                    Err(err) => Err(err),
                                }
                            }
                            Some(AudioKind::YoutubePlaylist) => {
                                match get_playlist_items_from_db(&uid, None, None).await {
                                    Ok(items) => Ok(MetadataQueryResult::ManyLocal(items)),
                                    Err(err) => Err(err),
                                }
                            }
                            None => Err(AppError::new(
                                AppErrorKind::LocalData,
                                "invalid audio uid",
                                &[&format!("UID: {uid}", uid = uid.0)],
                            )),
                        }
                    }
                    DownloadRequiredInformation::YoutubeVideo { url } => {
                        let uid = url.uid();
                        get_audio_metadata_from_db(&uid).await.map(|res| {
                            MetadataQueryResult::Single(
                                res.map(|md| LocalAudioMetadata::Found { metadata: md, uid })
                                    .unwrap_or(LocalAudioMetadata::NotFound {
                                        url: AudioUrl::Youtube(url.0),
                                    }),
                            )
                        })
                    }
                    DownloadRequiredInformation::YoutubePlaylist(YoutubePlaylistDownloadInfo {
                        video_urls,
                        playlist_url,
                    }) => {
                        let playlist_uid = playlist_url.uid();
                        store_playlist_if_not_exists(&playlist_uid).await?;

                        let mut metadata_list = Vec::with_capacity(video_urls.len());

                        for url in video_urls.iter() {
                            let youtube_url = YoutubeVideoUrl(url);
                            let audio_uid = youtube_url.uid();

                            let metadata = get_audio_metadata_from_db(&audio_uid).await?;
                            match metadata {
                                Some(metadata) => {
                                    metadata_list.push(LocalAudioMetadata::Found {
                                        metadata,
                                        uid: youtube_url.uid(),
                                    });

                                    store_playlist_item_relation_if_not_exists(
                                        &playlist_uid,
                                        &audio_uid,
                                    )
                                    .await?;
                                }
                                None => metadata_list.push(LocalAudioMetadata::NotFound {
                                    url: AudioUrl::Youtube(Arc::clone(youtube_url.0)),
                                }),
                            }
                        }

                        Ok(MetadataQueryResult::Many(LocalAudioMetadataList {
                            list_url: AudioUrl::Youtube(playlist_url.0),
                            metadata: metadata_list,
                        }))
                    }
                };

                query_res
            }
            .into_actor(self)
            .map(move |res, act, ctx| match res {
                Ok(MetadataQueryResult::Single(data)) => {
                    let msg = handle_add_single_queue_item(data, act, ctx.address().recipient());

                    if let Some(msg) = msg {
                        act.multicast_result(msg);
                    }
                }
                Ok(MetadataQueryResult::Many(LocalAudioMetadataList { list_url, metadata })) => {
                    let download_addr = act.downloader_addr.clone().recipient();

                    let audio_urls = metadata
                        .iter()
                        .filter_map(|data| {
                            if let LocalAudioMetadata::NotFound { url } = data {
                                Some(url.clone())
                            } else {
                                None
                            }
                        })
                        .collect();

                    let existing_metadata = metadata
                        .into_iter()
                        .filter_map(|data| match data {
                            LocalAudioMetadata::Found { metadata, uid } => Some((uid, metadata)),
                            _ => None,
                        })
                        .collect();

                    play_existing_playlist_items(act, existing_metadata);

                    request_download_of_missing_items(
                        Some(Arc::clone(&act.source_name)),
                        download_addr,
                        ctx.address().recipient(),
                        list_url,
                        audio_urls,
                    );
                }
                Ok(MetadataQueryResult::ManyLocal(items)) => {
                    play_existing_playlist_items(act, items);
                }
                Err(err_resp) => {
                    act.multicast(err_resp);
                }
            }),
        )
    }
}

fn play_existing_playlist_items(
    node: &mut AudioNode,
    metadata_list: Arc<[(ItemUid<Arc<str>>, AudioMetadata)]>,
) {
    if metadata_list.is_empty() {
        return;
    }

    for (uid, metadata) in metadata_list.iter().cloned() {
        let audio_item = AudioPlayerQueueItem {
            metadata,
            locator: uid.to_path_with_ext(),
            identifier: uid,
        };

        let _ = node.player.push_to_queue(audio_item);
    }

    node.multicast(AudioNodeInfoStreamMessage::Queue(extract_queue_metadata(
        node.player.queue(),
    )))
}

fn request_download_of_missing_items(
    source_name: Option<Arc<str>>,
    downloader_addr: Recipient<DownloadAudioRequest>,
    receiver_addr: Recipient<NotifyDownloadUpdate>,
    list_url: AudioUrl,
    audio_urls: Arc<[AudioUrl]>,
) {
    if audio_urls.is_empty() {
        return;
    }

    let urls = audio_urls
        .iter()
        .flat_map(|url| {
            if url.kind() != list_url.kind() {
                log::warn!(
                    "invalid url type '{:?}' in playlist of type '{:?}'",
                    url.kind(),
                    list_url.kind()
                );
                None
            } else {
                Some(url.inner())
            }
        })
        .collect();

    match list_url {
        AudioUrl::Youtube(url) => {
            let required_info =
                DownloadRequiredInformation::YoutubePlaylist(YoutubePlaylistDownloadInfo {
                    playlist_url: YoutubePlaylistUrl(url),
                    video_urls: urls,
                });

            let request = DownloadAudioRequest {
                source_name,
                addr: receiver_addr,
                required_info,
            };

            downloader_addr.do_send(request); // TODO handle mailbox full
        }
    }
}

impl AudioIdentifier {
    async fn into_required_info(self) -> Result<DownloadRequiredInformation, AppError> {
        let url = match self {
            Self::Local { uid } => return Ok(DownloadRequiredInformation::StoredLocally { uid }),
            Self::Youtube { url } => url,
        };

        let content_type = youtube_content_type(&*url);
        let url = clean_url(&url);

        match content_type {
            YoutubeContentType::Video => Ok(DownloadRequiredInformation::YoutubeVideo {
                url: YoutubeVideoUrl(url.into()),
            }),
            YoutubeContentType::Playlist => {
                let urls = match get_playlist_video_urls(url, yt_api_key()).await {
                    Ok(urls) => urls,
                    Err(err) => return Err(err),
                };

                Ok(DownloadRequiredInformation::YoutubePlaylist(
                    YoutubePlaylistDownloadInfo {
                        playlist_url: YoutubePlaylistUrl(url.into()),
                        video_urls: urls,
                    },
                ))
            }
            YoutubeContentType::Invalid => Err(AppError::new(
                AppErrorKind::Download,
                "invalid youtube video url",
                &[&format!("URL: {url}")],
            )),
        }
    }
}

fn handle_add_single_queue_item(
    data: LocalAudioMetadata,
    node: &mut AudioNode,
    node_addr: Recipient<NotifyDownloadUpdate>,
) -> Option<Result<AudioNodeInfoStreamMessage, AppError>> {
    match data {
        LocalAudioMetadata::Found { metadata, uid } => {
            if let Err(err) = node.player.push_to_queue(AudioPlayerQueueItem {
                metadata,
                locator: uid.to_path_with_ext(),
                identifier: uid,
            }) {
                return Some(Err(err.into_app_err(
                    "failed to auto play first song,",
                    AppErrorKind::Queue,
                    &[&format!("NODE_NAME: {name}", name = node.source_name)],
                )));
            }
        }
        LocalAudioMetadata::NotFound { url } => {
            let download_info = match url {
                AudioUrl::Youtube(url) => DownloadRequiredInformation::YoutubeVideo {
                    url: YoutubeVideoUrl(url),
                },
            };

            node.downloader_addr.do_send(DownloadAudioRequest {
                source_name: Some(Arc::clone(&node.source_name)),
                addr: node_addr,
                required_info: download_info,
            });

            return None;
        }
    }

    Some(Ok(AudioNodeInfoStreamMessage::Queue(
        extract_queue_metadata(node.player.queue()),
    )))
}
