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
pub enum AsyncAudioNodeCommand {
    AddQueueItem(AddQueueItemParams),
}

pub enum MetadataExists {
    Found {
        metadata: AudioMetaData,
        path: PathBuf,
    },
    NotFound {
        url: Arc<str>,
    },
}

impl Handler<AsyncAudioNodeCommand> for AudioNode {
    type Result = ResponseActFuture<Self, ()>;

    fn handle(&mut self, msg: AsyncAudioNodeCommand, _ctx: &mut Self::Context) -> Self::Result {
        enum MetadataQueryResult {
            Single(MetadataExists),
            Many(Vec<Option<AudioMetaData>>),
        }

        match msg {
            AsyncAudioNodeCommand::AddQueueItem(params) => {
                Box::pin(
                    async move {
                        let identifier = params.identifier.to_internal_identifier().await.unwrap();

                        // TODO: differentiate between video and playlist
                        //       - video:       use existing
                        //       - playlist:
                        //          1. query if playlist exists
                        //          2. create playlist table if not
                        //          3. create links to metadata for existing metadata
                        //          4. make request for remaining metadata

                        let query_res: Result<MetadataQueryResult, ErrorResponse> = match identifier
                        {
                            DownloadRequiredInformation::YoutubeVideo { url } => {
                                let uid = url.uid();
                                get_audio_metadata_from_db(&uid).await.map(|res| {
                                    MetadataQueryResult::Single(
                                        res.map(|md| MetadataExists::Found {
                                            metadata: md,
                                            path: url.to_path_with_ext(),
                                        })
                                        .unwrap_or(MetadataExists::NotFound { url: url.0 }),
                                    )
                                })
                            }
                            DownloadRequiredInformation::YoutubePlaylist(
                                YoutubePlaylistDownloadInfo {
                                    video_urls,
                                    playlist_url,
                                },
                            ) => {
                                let uid = playlist_url.uid();
                                let mut metadata_list = Vec::with_capacity(video_urls.len());

                                // TODO: figure out what is supposed to happen here
                                for url in video_urls {
                                    let _audio_identifier =
                                        DownloadRequiredInformation::YoutubeVideo {
                                            url: YoutubeVideoUrl(url),
                                        };

                                    let metadata = get_audio_metadata_from_db(&uid).await.unwrap();
                                    metadata_list.push(metadata);
                                }

                                Ok(MetadataQueryResult::Many(metadata_list))
                            }
                        };

                        query_res
                    }
                    .into_actor(self)
                    .map(move |res, act, ctx| {
                        match res {
                            Ok(MetadataQueryResult::Single(data)) => {
                                let msg = handle_add_single_queue_item(
                                    data,
                                    act,
                                    ctx.address().recipient(),
                                );

                                if let Some(msg) = msg {
                                    act.multicast_result(msg);
                                }
                            }
                            Ok(MetadataQueryResult::Many(_metadata_list)) => {
                                // TODO
                                // create playlist table if not exists

                                // create playlist items for existing items if not exists

                                // create new identifier with missing items & make download request

                                // play found metadata
                            }
                            Err(err_resp) => {
                                act.multicast(err_resp);
                            }
                        }
                    }),
                )
            } // _ => Box::pin(async { Ok(()) }.into_actor(self)),
        }
    }
}

impl DownloadIdentifierParam {
    async fn to_internal_identifier(self) -> anyhow::Result<DownloadRequiredInformation> {
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
