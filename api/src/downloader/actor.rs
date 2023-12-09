use crate::{
    audio_hosts::youtube::video::get_video_metadata,
    audio_playback::audio_item::AudioMetaData,
    db_pool,
    downloader::{
        download_identifier::{
            DownloadRequiredInformation, Identifier, YoutubePlaylistDownloadInfo,
            YoutubePlaylistUrl,
        },
        info::DownloadInfo,
    },
    utils::log_msg_received,
    yt_api_key, ErrorResponse, IntoErrResp,
};
use std::{collections::VecDeque, path::PathBuf, process::Command, sync::Arc, time::Duration};

use actix::{Actor, Context, Handler, Message, Recipient};
use actix_rt::Arbiter;
use anyhow::anyhow;
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

#[derive(Message)]
#[rtype(result = "()")]
pub struct NotifyDownloadUpdate {
    pub result: DownloadUpdate,
}

type SingleDownloadFinished =
    Result<(DownloadInfo, AudioMetaData, PathBuf), (DownloadInfo, ErrorResponse)>;

pub enum DownloadUpdate {
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
                let info = msg.identifier.into();
                msg.addr.do_send(NotifyDownloadUpdate {
                    result: DownloadUpdate::Queued(info),
                });
                queue.push_back(msg);
            }
            Err(err) => {
                let err_resp = err.into_err_resp("failed to add audio to download queue\nERROR:");
                let info = msg.identifier.into();
                msg.addr.do_send(NotifyDownloadUpdate {
                    result: DownloadUpdate::FailedToQueue((info, err_resp)),
                });
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
                process_single_video(&url, pool, &addr).await;
            }
            DownloadRequiredInformation::YoutubePlaylist(YoutubePlaylistDownloadInfo {
                ref playlist_url,
                video_urls,
            }) => {
                // TODO
                // - add db structure for playlists
                // - detect videos in playlist
                // - update playlist table with info

                let (videos_to_process, videos_for_next_batch) =
                    if MAX_CONSECUTIVE_BATCHES > video_urls.len() {
                        (video_urls.as_slice(), Default::default())
                    } else {
                        video_urls.split_at(MAX_CONSECUTIVE_BATCHES)
                    };

                for url in videos_to_process.to_owned() {
                    let tx = pool.begin().await.unwrap();

                    let info = DownloadInfo::yt_video(&url);
                    let video_url = YoutubeVideoUrl(&url);

                    let result = match download_youtube(&video_url, tx).await {
                        Ok(metadata) => Ok((info, metadata, video_url.to_path_with_ext())),
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

                    addr.do_send(NotifyDownloadUpdate {
                        result: DownloadUpdate::SingleFinished(result),
                    });
                }

                if videos_for_next_batch.is_empty() {
                    addr.do_send(NotifyDownloadUpdate {
                        result: DownloadUpdate::BatchUpdated {
                            batch: DownloadInfo::yt_playlist(&playlist_url.0, &video_urls),
                        },
                    });
                } else {
                    let next_batch =
                        DownloadRequiredInformation::YoutubePlaylist(YoutubePlaylistDownloadInfo {
                            playlist_url: YoutubePlaylistUrl(playlist_url.0.clone()),
                            video_urls: videos_for_next_batch.to_vec(),
                        });

                    addr.do_send(NotifyDownloadUpdate {
                        result: DownloadUpdate::BatchUpdated {
                            batch: DownloadInfo::yt_playlist(&playlist_url.0, &video_urls),
                        },
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

async fn process_single_video(
    url: &YoutubeVideoUrl<impl AsRef<str> + std::fmt::Display + std::fmt::Debug>,
    pool: &PgPool,
    addr: &Recipient<NotifyDownloadUpdate>,
) {
    let tx = pool.begin().await.unwrap();

    let info = DownloadInfo::yt_video(&url.0);

    let metadata = match download_youtube(url, tx).await {
        Ok(metadata) => metadata,
        Err(err) => {
            log::error!(
                "failed to download video, URL: {url}, ERROR: {err}",
                url = url.0
            );
            addr.do_send(NotifyDownloadUpdate {
                result: DownloadUpdate::SingleFinished(Err((
                    info,
                    ErrorResponse {
                        error: format!("failed to download video with url: {url}", url = url.0),
                    },
                ))),
            });
            return;
        }
    };

    addr.do_send(NotifyDownloadUpdate {
        result: DownloadUpdate::SingleFinished(Ok((info, metadata, url.to_path_with_ext()))),
    });
}

async fn download_youtube(
    url: &YoutubeVideoUrl<impl AsRef<str> + std::fmt::Debug>,
    mut tx: sqlx::Transaction<'_, sqlx::Postgres>,
) -> anyhow::Result<AudioMetaData> {
    let metadata: AudioMetaData = get_video_metadata(url.0.as_ref(), yt_api_key())
        .await?
        .into();

    let key = url.uid();
    sqlx::query!("INSERT INTO audio_metadata (identifier, name, author, duration, cover_art_url) values ($1, $2, $3, $4, $5)",
                    key,
                    metadata.name,
                    metadata.author,
                    metadata.duration,
                    metadata.cover_art_url
                )
                .execute(&mut *tx)
                .await?;

    let path = url.to_path_with_ext();
    if let Err(err) = download_youtube_audio(url.0.as_ref(), &path.to_string_lossy()) {
        if let Err(rollback_err) = tx.rollback().await {
            return Err(anyhow!("ERROR 1: {err}\nERROR 2: {rollback_err}"));
        }

        return Err(err);
    }

    tx.commit().await?;
    Ok(metadata)
}

fn download_youtube_audio(url: &str, download_location: &str) -> anyhow::Result<()> {
    let out = Command::new("yt-dlp")
        .args([
            "-f",
            "bestaudio",
            "-x",
            "--audio-format",
            "wav",
            "-o",
            download_location,
            url,
        ])
        .output()?;

    if out.status.code().unwrap_or(1) != 0 {
        return Err(anyhow!(String::from_utf8(out.stderr).unwrap_or(
            "failed to parse stderr of 'yt-dlp' command".to_owned()
        )));
    }

    Ok(())
}
