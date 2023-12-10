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
    commands::node_commands::{AddQueueItemParams, DownloadIdentifier},
    db_pool,
    downloader::{
        actor::{DownloadAudioRequest, NotifyDownloadUpdate},
        download_identifier::{Identifier, YoutubePlaylistUrl, YoutubeVideoUrl},
        DownloadRequiredInformation, YoutubePlaylistDownloadInfo,
    },
    node::node_server::{extract_queue_metadata, sync_actor::handle_add_single_queue_item},
    streams::node_streams::AudioNodeInfoStreamMessage,
    utils::log_msg_received,
    yt_api_key, ErrorResponse, IntoErrResp,
};

use super::AudioNode;

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
        url: UrlKind,
    },
}

#[derive(Debug)]
pub enum UrlKind {
    Youtube(Arc<str>),
}

type MetadataList = Vec<(Arc<str>, Option<AudioMetaData>)>;

#[derive(Debug, Message)]
#[rtype(result = "()")]
struct LocalAudioMetadataList {
    list_url: UrlKind,
    metadata: MetadataList,
}

impl Handler<AsyncAddQueueItem> for AudioNode {
    type Result = ResponseActFuture<Self, ()>;

    fn handle(&mut self, msg: AsyncAddQueueItem, _ctx: &mut Self::Context) -> Self::Result {
        log_msg_received(&self, &msg);

        enum MetadataQueryResult {
            Single(LocalAudioMetadata),
            Many(LocalAudioMetadataList),
        }

        Box::pin(
            async move {
                let identifier = msg.0.identifier.get_required_info().await.unwrap();

                let query_res: Result<MetadataQueryResult, ErrorResponse> = match identifier {
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
                                        url: UrlKind::Youtube(url.0),
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
                            let audio_uid = YoutubeVideoUrl(url.clone()).uid();

                            let metadata = get_audio_metadata_from_db(&audio_uid).await.unwrap();
                            if metadata.is_some() {
                                store_playlist_item_relation_if_not_exists(
                                    &playlist_uid,
                                    &audio_uid,
                                )
                                .await
                                .unwrap();
                            }

                            metadata_list.push((url, metadata));
                        }

                        Ok(MetadataQueryResult::Many(LocalAudioMetadataList {
                            list_url: UrlKind::Youtube(playlist_url.0),
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

                    play_existing_playlist_items(act, &metadata);

                    request_download_of_missing_items(
                        download_addr,
                        ctx.address().recipient(),
                        list_url,
                        &metadata,
                    );
                }
                Err(err_resp) => {
                    act.multicast(err_resp);
                }
            }),
        )
    } // _ => Box::pin(async { Ok(()) }.into_actor(self)),
}

fn play_existing_playlist_items(node: &mut AudioNode, metadata_list: &MetadataList) {
    let prev_queue_len = node.player.queue().len();
    for (url, metadata) in metadata_list {
        if let Some(data) = metadata {
            let audio_item = AudioPlayerQueueItem {
                metadata: data.clone(),
                locator: YoutubeVideoUrl(url).to_path_with_ext(),
            };

            // TODO handle error
            node.player.push_to_queue(audio_item).unwrap();
        }
    }

    if prev_queue_len != node.player.queue().len() {
        node.multicast(AudioNodeInfoStreamMessage::Queue(extract_queue_metadata(
            node.player.queue(),
        )))
    }
}

fn request_download_of_missing_items(
    downloader_addr: Recipient<DownloadAudioRequest>,
    receiver_addr: Recipient<NotifyDownloadUpdate>,
    list_url: UrlKind,
    metadata_list: &MetadataList,
) {
    match list_url {
        UrlKind::Youtube(url) => {
            let video_urls: Vec<_> = metadata_list
                .iter()
                .filter_map(|(url, metadata)| {
                    if metadata.is_none() {
                        Some(Arc::clone(url))
                    } else {
                        None
                    }
                })
                .collect();

            if video_urls.is_empty() {
                return;
            }

            let required_info =
                DownloadRequiredInformation::YoutubePlaylist(YoutubePlaylistDownloadInfo {
                    playlist_url: YoutubePlaylistUrl(url),
                    video_urls,
                });

            let request = DownloadAudioRequest {
                addr: receiver_addr,
                required_info,
            };

            downloader_addr.do_send(request); // TODO handle mailbox full
        }
    }
}

impl DownloadIdentifier {
    async fn get_required_info(self) -> anyhow::Result<DownloadRequiredInformation> {
        let url = match self {
            Self::Youtube { url } => url,
        };

        let content_type = youtube_content_type(url.as_str());

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
