use crate::{
    audio_playback::audio_item::AudioMetaData,
    db_pool,
    downloader::{
        download_identifier::{
            DownloadRequiredInformation, Identifier, YoutubePlaylistDownloadInfo,
        },
        info::DownloadInfo,
        youtube::{download_and_store_youtube_audio_with_metadata, process_single_youtube_video},
    },
    node::node_server::async_actor::store_playlist_item_relation_if_not_exists,
    utils::log_msg_received,
    ErrorResponse, IntoErrResp,
};
use std::{collections::VecDeque, path::PathBuf, sync::Arc, time::Duration};

use actix::{Actor, Context, Handler, Message, Recipient};
use actix_rt::Arbiter;
use sqlx::PgPool;
use tokio::sync::Mutex;

use super::download_identifier::YoutubeVideoUrl;

const MAX_CONSECUTIVE_BATCHES: usize = 10;

pub struct AudioDownloader {
    download_thread: Arbiter,
    queue: Arc<Mutex<VecDeque<DownloadAudioRequest>>>,
}

#[derive(Debug, Message)]
#[rtype(result = "()")]
pub struct DownloadAudioRequest {
    pub addr: Recipient<NotifyDownloadUpdate>,
    pub identifier: DownloadRequiredInformation,
}

type SingleDownloadFinished =
    Result<(DownloadInfo, AudioMetaData, PathBuf), (DownloadInfo, ErrorResponse)>;

#[derive(Message)]
#[rtype(result = "()")]
pub enum NotifyDownloadUpdate {
    Queued(DownloadInfo),
    FailedToQueue((DownloadInfo, ErrorResponse)),
    SingleFinished(SingleDownloadFinished),
    BatchUpdated { batch: DownloadInfo },
}

impl AudioDownloader {
    pub fn new(download_thread: Arbiter) -> Self {
        Self {
            download_thread,
            queue: Default::default(),
        }
    }
}

impl Actor for AudioDownloader {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        log::info!("stared new 'AudioDownloader', CONTEXT: {ctx:?}");

        let queue = self.queue.clone();

        self.download_thread.spawn(async move {
            loop {
                process_queue(queue.clone(), db_pool()).await;
                actix_rt::time::sleep(Duration::from_secs(1)).await;
            }
        });
    }
}

impl Handler<DownloadAudioRequest> for AudioDownloader {
    type Result = ();

    fn handle(&mut self, msg: DownloadAudioRequest, _ctx: &mut Self::Context) -> Self::Result {
        log_msg_received(&self, &msg);

        match self.queue.try_lock() {
            Ok(mut queue) => {
                let info = (&msg.identifier).into();
                msg.addr.do_send(NotifyDownloadUpdate::Queued(info));

                queue.push_back(msg);
            }
            Err(err) => {
                let err_resp = err.into_err_resp("failed to add audio to download queue\nERROR:");
                let info = msg.identifier.into();
                msg.addr
                    .do_send(NotifyDownloadUpdate::FailedToQueue((info, err_resp)));
            }
        }
    }
}

async fn process_queue(queue: Arc<Mutex<VecDeque<DownloadAudioRequest>>>, pool: &PgPool) {
    let mut queue = queue.lock().await;

    if let Some(req) = queue.pop_front() {
        let DownloadAudioRequest {
            addr, identifier, ..
        } = req;
        log::info!("download for {identifier:?} has started");

        match identifier {
            DownloadRequiredInformation::YoutubeVideo { url } => {
                process_single_youtube_video(&url, pool, &addr).await;
            }
            DownloadRequiredInformation::YoutubePlaylist(YoutubePlaylistDownloadInfo {
                ref playlist_url,
                video_urls,
            }) => {
                let (videos_to_process, videos_for_next_batch) =
                    if MAX_CONSECUTIVE_BATCHES > video_urls.len() {
                        (video_urls.as_slice(), Default::default())
                    } else {
                        video_urls.split_at(MAX_CONSECUTIVE_BATCHES)
                    };

                for url in videos_to_process.to_owned() {
                    let tx = pool.begin().await.unwrap();

                    let info = DownloadInfo::yt_video_from_arc(&url);
                    let video_url = YoutubeVideoUrl(&url);

                    let result = match download_and_store_youtube_audio_with_metadata(
                        &video_url, tx,
                    )
                    .await
                    {
                        Ok(metadata) => {
                            store_playlist_item_relation_if_not_exists(
                                &playlist_url.uid(),
                                &video_url.uid(),
                            )
                            .await
                            .unwrap();

                            Ok((info, metadata, video_url.to_path_with_ext()))
                        }
                        Err(err) => {
                            log::error!("failed to download video, URL: {url}, ERROR: {err}");
                            Err((
                                info,
                                ErrorResponse {
                                    error: format!("failed to download video with url: {url}"),
                                },
                            ))
                        }
                    };

                    addr.do_send(NotifyDownloadUpdate::SingleFinished(result));
                }

                if videos_for_next_batch.is_empty() {
                    addr.do_send(NotifyDownloadUpdate::BatchUpdated {
                        batch: DownloadInfo::yt_playlist_from_arc(
                            &playlist_url.0,
                            &videos_for_next_batch,
                        ),
                    });
                } else {
                    let next_batch =
                        DownloadRequiredInformation::YoutubePlaylist(YoutubePlaylistDownloadInfo {
                            playlist_url: playlist_url.clone(),
                            video_urls: videos_for_next_batch.to_vec(),
                        });

                    addr.do_send(NotifyDownloadUpdate::BatchUpdated {
                        batch: DownloadInfo::yt_playlist_from_arc(
                            &playlist_url.0,
                            &videos_for_next_batch,
                        ),
                    });

                    queue.push_back(DownloadAudioRequest {
                        addr,
                        identifier: next_batch,
                    });
                }
            }
        }
    }
}
