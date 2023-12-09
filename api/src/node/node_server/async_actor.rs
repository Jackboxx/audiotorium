use std::{path::PathBuf, sync::Arc};

use actix::{ActorFutureExt, AsyncContext, Handler, Message, ResponseActFuture, WrapFuture};
use anyhow::anyhow;

use crate::{
    audio_hosts::youtube::{
        playlist::get_playlist_video_urls, youtube_content_type, YoutubeContentType,
    },
    audio_playback::audio_item::AudioMetaData,
    commands::node_commands::{AddQueueItemParams, DownloadIdentifierParam},
    db_pool,
    downloader::download_identifier::{
        DownloadRequiredInformation, Identifier, YoutubePlaylistDownloadInfo, YoutubePlaylistUrl,
        YoutubeVideoUrl,
    },
    node::node_server::sync_actor::handle_add_single_queue_item,
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

#[derive(Debug, Message)]
#[rtype(type = "()")]
struct LocalAudioMetadataList {
    list_url: UrlKind,
    metadata: Vec<(Arc<str>, Option<AudioMetaData>)>,
}

impl Handler<AsyncAddQueueItem> for AudioNode {
    type Result = ResponseActFuture<Self, ()>;

    fn handle(&mut self, msg: AsyncAddQueueItem, _ctx: &mut Self::Context) -> Self::Result {
        enum MetadataQueryResult {
            Single(LocalAudioMetadata),
            Many(LocalAudioMetadataList),
        }

        Box::pin(
            async move {
                let identifier = msg.0.identifier.get_required_info().await.unwrap();

                // TODO: differentiate between video and playlist
                //       - video:       use existing
                //       - playlist:
                //          1. query if playlist exists
                //          2. create playlist table if not
                //          3. create links to metadata for existing metadata
                //          4. make request for remaining metadata

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
                        let uid = playlist_url.uid();
                        let mut metadata_list = Vec::with_capacity(video_urls.len());

                        for url in video_urls {
                            let metadata = get_audio_metadata_from_db(&uid).await.unwrap();
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
                Ok(MetadataQueryResult::Many(local_list)) => ctx.notify(local_list),
                Err(err_resp) => {
                    act.multicast(err_resp);
                }
            }),
        )
    } // _ => Box::pin(async { Ok(()) }.into_actor(self)),
}

impl Handler<LocalAudioMetadataList> for AudioNode {
    type Result = ResponseActFuture<Self, ()>;

    fn handle(&mut self, msg: LocalAudioMetadataList, ctx: &mut Self::Context) -> Self::Result {
        // TODO
        todo!()
    }
}

impl DownloadIdentifierParam {
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
