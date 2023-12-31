use crate::{
    audio_playback::audio_item::AudioMetadata,
    database::store_data::{
        store_playlist_if_not_exists, store_playlist_item_relation_if_not_exists,
    },
    db_pool,
    downloader::{
        download_identifier::Identifier,
        info::DownloadInfo,
        youtube::{download_and_store_youtube_audio_with_metadata, process_single_youtube_video},
        DownloadRequiredInformation, YoutubePlaylistDownloadInfo,
    },
    error::{AppError, AppErrorKind, IntoAppError},
    node::node_server::SourceName,
    state_storage::restore_state_actor::{DownloadQueueStateUpdateMessage, RestoreStateActor},
    utils::log_msg_received,
};
use std::{collections::VecDeque, sync::Arc, time::Duration};

use actix::{Actor, Addr, Context, Handler, Message, Recipient};
use actix_rt::Arbiter;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tokio::sync::Mutex;

use super::{
    download_identifier::{ItemUid, YoutubeVideoUrl},
    info::OptionalDownloadInfo,
};

const MAX_CONSECUTIVE_BATCHES: usize = 10;

pub struct AudioDownloader {
    download_thread: Arbiter,
    queue: Arc<Mutex<VecDeque<DownloadAudioRequest>>>,
    restore_state_addr: Addr<RestoreStateActor>,
}

#[derive(Debug, Clone, Message)]
#[rtype(result = "()")]
pub struct DownloadAudioRequest {
    pub source_name: Option<SourceName>,
    pub addr: Recipient<NotifyDownloadUpdate>,
    pub required_info: DownloadRequiredInformation,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SerializableDownloadAudioRequest {
    pub source_name: Option<SourceName>,
    pub required_info: DownloadRequiredInformation,
}

type SingleDownloadFinished =
    Result<(DownloadInfo, AudioMetadata, ItemUid<Arc<str>>), (DownloadInfo, AppError)>;

#[derive(Debug, Message)]
#[rtype(result = "()")]
pub enum NotifyDownloadUpdate {
    Queued(DownloadInfo),
    FailedToQueue((DownloadInfo, AppError)),
    SingleFinished(SingleDownloadFinished),
    BatchUpdated { batch: DownloadInfo },
    BatchDownloadFailedToStart((DownloadInfo, AppError)),
}

#[derive(Debug, Message)]
#[rtype(result = "()")]
pub struct RestoreQueue(pub Vec<DownloadAudioRequest>);

impl AudioDownloader {
    pub fn new(download_thread: Arbiter, restore_state_addr: Addr<RestoreStateActor>) -> Self {
        Self {
            download_thread,
            restore_state_addr,
            queue: Default::default(),
        }
    }
}

impl Actor for AudioDownloader {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        log::info!("stared new 'AudioDownloader', CONTEXT: {ctx:?}");

        let queue = self.queue.clone();
        let restore_state_addr = self.restore_state_addr.clone().recipient();

        self.download_thread.spawn(async move {
            loop {
                process_queue(queue.clone(), db_pool(), &restore_state_addr).await;
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
                let info: OptionalDownloadInfo = (&msg.required_info).into();

                if let Some(info) = info.into() {
                    msg.addr.do_send(NotifyDownloadUpdate::Queued(info));
                }

                queue.push_back(msg);
            }
            Err(err) => {
                let err_resp = err.into_app_err(
                    "failed to queue audio for download",
                    AppErrorKind::Download,
                    &[],
                );

                let info: OptionalDownloadInfo = msg.required_info.into();
                if let Some(info) = info.into() {
                    msg.addr
                        .do_send(NotifyDownloadUpdate::FailedToQueue((info, err_resp)));
                }
            }
        }
    }
}

impl Handler<RestoreQueue> for AudioDownloader {
    type Result = ();

    fn handle(&mut self, msg: RestoreQueue, _ctx: &mut Self::Context) -> Self::Result {
        log_msg_received(&self, &msg);

        match self.queue.try_lock() {
            Ok(mut queue) => {
                let len = queue.len();
                queue.drain(..);
                queue.append(&mut msg.0.into_iter().collect());

                for item in queue.iter() {
                    let info: OptionalDownloadInfo = (&item.required_info).into();
                    if let Some(info) = info.into() {
                        item.addr.do_send(NotifyDownloadUpdate::Queued(info));
                    }
                }

                log::info!("restored {len} items to download queue");
            }
            Err(err) => log::error!("failed to restore download queue\nERROR: {err}"),
        };
    }
}

async fn process_queue(
    queue: Arc<Mutex<VecDeque<DownloadAudioRequest>>>,
    pool: &PgPool,
    restore_state_addr: &Recipient<DownloadQueueStateUpdateMessage>,
) {
    let mut queue = queue.lock().await;

    restore_state_addr.do_send(DownloadQueueStateUpdateMessage(
        queue
            .iter()
            .map(|item| SerializableDownloadAudioRequest {
                source_name: item.source_name.clone(),
                required_info: item.required_info.clone(),
            })
            .collect(),
    ));

    if let Some(req) = queue.pop_front() {
        let DownloadAudioRequest {
            source_name,
            addr,
            required_info,
        } = req;
        log::info!("download for {required_info:?} has started");

        match required_info {
            DownloadRequiredInformation::StoredLocally { uid } => {
                log::warn!("downloader received request for locally stored item with uid '{uid}'");
            }
            DownloadRequiredInformation::YoutubeVideo { url } => {
                process_single_youtube_video(&url, pool, &addr).await;
            }
            DownloadRequiredInformation::YoutubePlaylist(YoutubePlaylistDownloadInfo {
                ref playlist_url,
                video_urls,
            }) => {
                let playlist_uid = playlist_url.uid();
                match store_playlist_if_not_exists(&playlist_uid).await {
                    Ok(_) => {}
                    Err(err) => {
                        addr.do_send(NotifyDownloadUpdate::BatchDownloadFailedToStart((
                            DownloadInfo::yt_playlist_from_arc(&playlist_url.0, &video_urls),
                            err,
                        )));
                        return;
                    }
                }

                let (videos_to_process, videos_for_next_batch) =
                    if MAX_CONSECUTIVE_BATCHES > video_urls.len() {
                        (video_urls.as_ref(), Default::default())
                    } else {
                        video_urls.split_at(MAX_CONSECUTIVE_BATCHES)
                    };

                for url in videos_to_process {
                    let info = DownloadInfo::yt_video_from_arc(url);

                    let tx = match pool.begin().await.into_app_err(
                        "failed to start transaction",
                        AppErrorKind::Database,
                        &[],
                    ) {
                        Ok(tx) => tx,
                        Err(err) => {
                            addr.do_send(NotifyDownloadUpdate::FailedToQueue((info, err)));
                            return;
                        }
                    };

                    let video_url = YoutubeVideoUrl(&url);

                    let result = match download_and_store_youtube_audio_with_metadata(
                        &video_url, tx,
                    )
                    .await
                    {
                        Ok(metadata) => {
                            match store_playlist_item_relation_if_not_exists(
                                &playlist_url.uid(),
                                &video_url.uid(),
                            )
                            .await
                            {
                                Ok(()) => Ok((info, metadata, video_url.uid())),
                                Err(err) => Err((info, err)),
                            }
                        }
                        Err(err) => Err((info, err)),
                    };

                    addr.do_send(NotifyDownloadUpdate::SingleFinished(result));
                }

                if videos_for_next_batch.is_empty() {
                    addr.do_send(NotifyDownloadUpdate::BatchUpdated {
                        batch: DownloadInfo::yt_playlist_from_arc(
                            &playlist_url.0,
                            videos_for_next_batch,
                        ),
                    });
                } else {
                    let next_batch =
                        DownloadRequiredInformation::YoutubePlaylist(YoutubePlaylistDownloadInfo {
                            playlist_url: playlist_url.clone(),
                            video_urls: videos_for_next_batch.into(),
                        });

                    addr.do_send(NotifyDownloadUpdate::BatchUpdated {
                        batch: DownloadInfo::yt_playlist_from_arc(
                            &playlist_url.0,
                            videos_for_next_batch,
                        ),
                    });

                    queue.push_back(DownloadAudioRequest {
                        source_name,
                        addr,
                        required_info: next_batch,
                    });
                }
            }
        }
    }
}

impl From<DownloadAudioRequest> for SerializableDownloadAudioRequest {
    fn from(value: DownloadAudioRequest) -> Self {
        Self {
            source_name: value.source_name,
            required_info: value.required_info,
        }
    }
}
