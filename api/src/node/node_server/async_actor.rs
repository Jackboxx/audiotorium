use std::{path::PathBuf, sync::Arc};

use actix::{
    ActorFutureExt, AsyncContext, Handler, Message, Recipient, ResponseActFuture, WrapFuture,
};
use anyhow::anyhow;

use crate::{
    audio_hosts::youtube::{
        playlist::get_playlist_video_urls, youtube_content_type, YoutubeContentType,
    },
    audio_playback::audio_item::{AudioMetaData, AudioPlayerQueueItem},
    commands::node_commands::{AddQueueItemParams, AudioIdentifier},
    db_pool,
    downloader::{
        actor::{DownloadAudioRequest, NotifyDownloadUpdate},
        download_identifier::{
            AudioKind, AudioUid, Identifier, YoutubePlaylistUrl, YoutubeVideoUrl,
        },
        DownloadRequiredInformation, YoutubePlaylistDownloadInfo,
    },
    node::node_server::{extract_queue_metadata, sync_actor::handle_add_single_queue_item},
    streams::node_streams::AudioNodeInfoStreamMessage,
    utils::log_msg_received,
    yt_api_key, ErrorResponse, IntoErrResp,
};

use super::{clean_url, AudioNode, AudioUrl};

#[derive(Debug, Clone, Message)]
#[rtype(result = "()")]
pub struct AsyncAddQueueItem(pub AddQueueItemParams);

#[derive(Debug)]
pub enum LocalAudioMetadata {
    Found {
        metadata: AudioMetaData,
        path: PathBuf,
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
            ManyLocal(Vec<(AudioUid<Arc<str>>, AudioMetaData)>),
        }

        Box::pin(
            async move {
                let identifier = msg.0.identifier.into_required_info().await.unwrap();

                let query_res: Result<MetadataQueryResult, ErrorResponse> = match identifier {
                    DownloadRequiredInformation::StoredLocally { uid } => {
                        let uid = AudioUid(uid);
                        let kind = AudioKind::from_uid(&uid);

                        match kind {
                            Some(AudioKind::YoutubeVideo) => {
                                match get_audio_metadata_from_db(&uid.0).await {
                                    Ok(Some(metadata)) => {
                                        Ok(MetadataQueryResult::Single(LocalAudioMetadata::Found {
                                            metadata,
                                            path: uid.to_path_with_ext(),
                                        }))
                                    }
                                    Ok(None) => {
                                        log::error!("no local data found for uid '{uid:?}'");
                                        Err(ErrorResponse {
                                            error: "no local audio data found".to_owned(),
                                        })
                                    }
                                    Err(err) => {
                                        log::error!("failed to get local audio data for uid '{uid:?}, ERROR: {err:?}'");
                                        Err(err)
                                    },
                                }
                            }
                            Some(AudioKind::YoutubePlaylist) => {
                                match get_playlist_items_from_db(&uid.0).await {
                                    Ok(items) => Ok(MetadataQueryResult::ManyLocal(items)),
                                    Err(err) => {
                                        log::error!("failed to get local playlist audio data for uid '{uid:?}, ERROR: {err:?}'");
                                        Err(err)
                                    },
                                }
                            }
                            None => {
                                log::error!("invalid audio uid '{uid:?}'");
                                Err(ErrorResponse {
                                    error: "invalid audio uid".to_owned(),
                                })
                            },
                        }
                    }
                    DownloadRequiredInformation::YoutubeVideo { url } => {
                        let uid = url.uid();
                        get_audio_metadata_from_db(&uid).await.map(|res| {
                            MetadataQueryResult::Single(
                                res.map(|md| LocalAudioMetadata::Found {
                                    metadata: md,
                                    path: url.to_path_with_ext(),
                                })
                                .unwrap_or(
                                    LocalAudioMetadata::NotFound {
                                        url: AudioUrl::Youtube(url.0),
                                    },
                                ),
                            )
                        })
                    }
                    DownloadRequiredInformation::YoutubePlaylist(YoutubePlaylistDownloadInfo {
                        video_urls,
                        playlist_url,
                    }) => {
                        let playlist_uid = playlist_url.uid();
                        store_playlist_if_not_exists(&playlist_uid).await.unwrap();

                        let mut metadata_list = Vec::with_capacity(video_urls.len());

                        for url in video_urls {
                            let youtube_url = YoutubeVideoUrl(url);
                            let audio_uid = youtube_url.uid();

                            let metadata = get_audio_metadata_from_db(&audio_uid).await.unwrap();
                            match metadata {
                                Some(metadata) => {
                                    metadata_list.push(LocalAudioMetadata::Found {
                                        metadata,
                                        path: youtube_url.to_path_with_ext(),
                                    });
                                    store_playlist_item_relation_if_not_exists(
                                        &playlist_uid,
                                        &audio_uid,
                                    )
                                    .await
                                    .unwrap();
                                }
                                None => metadata_list.push(LocalAudioMetadata::NotFound {
                                    url: AudioUrl::Youtube(youtube_url.0),
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
                            LocalAudioMetadata::Found { metadata, path } => Some((path, metadata)),
                            _ => None,
                        })
                        .collect();

                    play_existing_playlist_items(act, existing_metadata);

                    request_download_of_missing_items(
                        download_addr,
                        ctx.address().recipient(),
                        list_url,
                        audio_urls,
                    );
                }
                Ok(MetadataQueryResult::ManyLocal(items)) => {
                    play_existing_playlist_items(
                        act,
                        items
                            .into_iter()
                            .map(|(uid, m)| (uid.to_path_with_ext(), m))
                            .collect(),
                    );
                }
                Err(err_resp) => {
                    act.multicast(err_resp);
                }
            }),
        )
    } // _ => Box::pin(async { Ok(()) }.into_actor(self)),
}

fn play_existing_playlist_items(
    node: &mut AudioNode,
    metadata_list: Vec<(PathBuf, AudioMetaData)>,
) {
    if metadata_list.is_empty() {
        return;
    }

    for (path, metadata) in metadata_list {
        let audio_item = AudioPlayerQueueItem {
            metadata,
            locator: path,
        };

        // TODO handle error
        node.player.push_to_queue(audio_item).unwrap();
    }

    node.multicast(AudioNodeInfoStreamMessage::Queue(extract_queue_metadata(
        node.player.queue(),
    )))
}

fn request_download_of_missing_items(
    downloader_addr: Recipient<DownloadAudioRequest>,
    receiver_addr: Recipient<NotifyDownloadUpdate>,
    list_url: AudioUrl,
    audio_urls: Vec<AudioUrl>,
) {
    if audio_urls.is_empty() {
        return;
    }

    let urls = audio_urls
        .into_iter()
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
                addr: receiver_addr,
                required_info,
            };

            downloader_addr.do_send(request); // TODO handle mailbox full
        }
    }
}

impl AudioIdentifier {
    async fn into_required_info(self) -> anyhow::Result<DownloadRequiredInformation> {
        let url = match self {
            Self::Local { uid } => {
                return Ok(DownloadRequiredInformation::StoredLocally { uid: uid.into() })
            }
            Self::Youtube { url } => url,
        };

        let content_type = youtube_content_type(url.as_str());
        let url = clean_url(&url);

        match content_type {
            YoutubeContentType::Video => Ok(DownloadRequiredInformation::YoutubeVideo {
                url: YoutubeVideoUrl(url.into()),
            }),
            YoutubeContentType::Playlist => {
                let urls = match get_playlist_video_urls(&url, yt_api_key()).await {
                    Ok(urls) => urls,
                    Err(err) => {
                        log::error!("failed to get playlist information for youtube playlist, URL: {url}, ERROR: {err}");
                        return Err(anyhow!(
                            "failed to get playlist information for youtube playlist, URL: {url}"
                        ));
                    }
                };

                Ok(DownloadRequiredInformation::YoutubePlaylist(
                    YoutubePlaylistDownloadInfo {
                        playlist_url: YoutubePlaylistUrl(url.into()),
                        video_urls: urls.into_iter().map(Into::into).collect(),
                    },
                ))
            }
            YoutubeContentType::Invalid => Err(anyhow!("invalid youtube video url, URL: {url}")),
        }
    }
}

async fn get_audio_metadata_from_db(uid: &str) -> Result<Option<AudioMetaData>, ErrorResponse> {
    sqlx::query_as!(
        AudioMetaData,
        "SELECT name, author, duration, cover_art_url FROM audio_metadata where identifier = $1",
        uid
    )
    .fetch_optional(db_pool())
    .await
    .into_err_resp("")
}

async fn get_playlist_items_from_db(
    playlist_uid: &str,
) -> Result<Vec<(AudioUid<Arc<str>>, AudioMetaData)>, ErrorResponse> {
    struct QueryResult {
        identifier: Arc<str>,
        name: Option<String>,
        author: Option<String>,
        duration: Option<i64>,
        cover_art_url: Option<String>,
    }

    sqlx::query_as!(
        QueryResult,
        "SELECT audio.identifier, audio.name, audio.author, audio.duration, audio.cover_art_url 
            FROM audio_metadata audio
        INNER JOIN audio_playlist_item items 
            ON items.playlist_identifier = $1",
        playlist_uid
    )
    .fetch_all(db_pool())
    .await
    .map(|vec| {
        vec.into_iter()
            .map(|res| {
                (
                    AudioUid(res.identifier),
                    AudioMetaData {
                        name: res.name,
                        author: res.author,
                        duration: res.duration,
                        cover_art_url: res.cover_art_url,
                    },
                )
            })
            .collect()
    })
    .into_err_resp("")
}

async fn store_playlist_if_not_exists(uid: &str) -> Result<(), ErrorResponse> {
    let mut tx = db_pool().begin().await.into_err_resp("")?;

    sqlx::query!(
        "INSERT INTO audio_playlist
        (identifier) VALUES ($1)
        ON CONFLICT DO NOTHING",
        uid
    )
    .execute(&mut *tx)
    .await
    .into_err_resp("")?;

    tx.commit().await.into_err_resp("")
}

pub async fn store_playlist_item_relation_if_not_exists(
    playlist_uid: &str,
    audio_uid: &str,
) -> Result<(), ErrorResponse> {
    let mut tx = db_pool().begin().await.into_err_resp("")?;

    sqlx::query!(
        "INSERT INTO audio_playlist_item
        (playlist_identifier, item_identifier) VALUES ($1, $2)
        ON CONFLICT DO NOTHING",
        playlist_uid,
        audio_uid
    )
    .execute(&mut *tx)
    .await
    .into_err_resp("")?;

    tx.commit().await.into_err_resp("")
}
