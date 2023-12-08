use crate::{
    audio_hosts::youtube::video::get_video_metadata, audio_playback::audio_item::AudioMetaData,
    db_pool, utils::log_msg_received, yt_api_key, ErrorResponse, IntoErrResp,
};
use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
    time::Duration,
};

use actix::{Actor, Context, Handler, Message, Recipient};
use actix_rt::Arbiter;
use anyhow::anyhow;
use serde::Serialize;
use sqlx::PgPool;
use tokio::sync::Mutex;
use ts_rs::TS;

#[cfg(not(debug_assertions))]
pub const AUDIO_DIR: &str = "audio";

#[cfg(debug_assertions)]
pub const AUDIO_DIR: &str = "audio-dev";

const MAX_CONSECUTIVE_BATCHES: usize = 10;

pub struct AudioDownloader {
    download_thread: Arbiter,
    queue: Arc<Mutex<VecDeque<DownloadAudioRequest>>>,
}

#[derive(Debug, Clone)]
pub enum DownloadIdentifier {
    YoutubeVideo { url: String },
    YoutubePlaylist(YoutubePlaylistDownloadInfo),
}

#[derive(Debug, Clone)]
pub struct YoutubePlaylistDownloadInfo {
    pub playlist_url: String,
    pub video_urls: Vec<String>,
}

#[derive(Debug, Clone, Eq, Serialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "../app/src/api-types/")]
pub enum DownloadInfo {
    YoutubeVideo {
        url: String,
    },
    YoutubePlaylist {
        playlist_url: String,
        video_urls: Vec<String>,
    },
}

impl std::hash::Hash for DownloadInfo {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Self::YoutubeVideo { url } => url.hash(state),
            Self::YoutubePlaylist { playlist_url, .. } => playlist_url.hash(state),
        };
    }
}

impl PartialEq for DownloadInfo {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (DownloadInfo::YoutubeVideo { url }, DownloadInfo::YoutubeVideo { url: url_other }) => {
                url.eq(url_other)
            }
            (
                DownloadInfo::YoutubePlaylist { playlist_url, .. },
                DownloadInfo::YoutubePlaylist {
                    playlist_url: playlist_url_other,
                    ..
                },
            ) => playlist_url.eq(playlist_url_other),
            _ => false,
        }
    }
}

impl From<DownloadIdentifier> for DownloadInfo {
    fn from(value: DownloadIdentifier) -> Self {
        match value {
            DownloadIdentifier::YoutubeVideo { url } => DownloadInfo::YoutubeVideo { url },
            DownloadIdentifier::YoutubePlaylist(YoutubePlaylistDownloadInfo {
                playlist_url,
                video_urls,
            }) => DownloadInfo::YoutubePlaylist {
                playlist_url,
                video_urls,
            },
        }
    }
}

#[derive(Debug, Clone, Message)]
#[rtype(result = "()")]
pub struct DownloadAudioRequest {
    pub addr: Recipient<NotifyDownloadUpdate>,
    pub identifier: DownloadIdentifier,
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

impl DownloadIdentifier {
    pub fn uid(&self) -> String {
        match self {
            Self::YoutubeVideo { url, .. } => {
                let prefix = "youtube_audio_";
                let hex_url = hex::encode(url);

                format!("{prefix}{hex_url}")
            }
            Self::YoutubePlaylist(YoutubePlaylistDownloadInfo { playlist_url, .. }) => {
                let prefix = "youtube_audio_playlist_";
                let hex_url = hex::encode(playlist_url);

                format!("{prefix}{hex_url}")
            }
        }
    }

    pub fn url(&self) -> &str {
        match self {
            Self::YoutubeVideo { url } => &url,
            Self::YoutubePlaylist(YoutubePlaylistDownloadInfo { playlist_url, .. }) => {
                &playlist_url
            }
        }
    }

    pub fn to_path(&self) -> PathBuf {
        Path::new(AUDIO_DIR).join(self.uid())
    }

    pub fn to_path_with_ext(&self) -> PathBuf {
        self.to_path().with_extension("wav")
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
                let info = msg.identifier.clone().into();
                msg.addr.do_send(NotifyDownloadUpdate {
                    result: DownloadUpdate::Queued(info),
                });
                queue.push_back(msg);
            }
            Err(err) => {
                let err_resp = err.into_err_resp("failed to add audio to download queue\nERROR:");
                let info = msg.identifier.clone().into();
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
            DownloadIdentifier::YoutubeVideo { .. } => {
                process_single_video(pool, &addr, identifier).await;
            }
            DownloadIdentifier::YoutubePlaylist(YoutubePlaylistDownloadInfo {
                playlist_url,
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

                    let identifier = DownloadIdentifier::YoutubeVideo { url: url.clone() };
                    let info = DownloadInfo::YoutubeVideo { url: url.clone() };

                    let result = match download_youtube(&identifier, tx).await {
                        Ok(metadata) => Ok((info, metadata, identifier.to_path_with_ext())),
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
                            batch: DownloadInfo::YoutubePlaylist {
                                playlist_url,
                                video_urls: vec![],
                            },
                        },
                    });
                } else {
                    let next_batch =
                        DownloadIdentifier::YoutubePlaylist(YoutubePlaylistDownloadInfo {
                            playlist_url: playlist_url.clone(),
                            video_urls: videos_for_next_batch.to_vec(),
                        });

                    addr.do_send(NotifyDownloadUpdate {
                        result: DownloadUpdate::BatchUpdated {
                            batch: DownloadInfo::YoutubePlaylist {
                                playlist_url,
                                video_urls: videos_for_next_batch.to_vec(),
                            },
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
    pool: &PgPool,
    addr: &Recipient<NotifyDownloadUpdate>,
    identifier: DownloadIdentifier,
) {
    let tx = pool.begin().await.unwrap();

    let url = identifier.url();
    let info = DownloadInfo::YoutubeVideo {
        url: url.to_owned(),
    };

    let metadata = match download_youtube(&identifier, tx).await {
        Ok(metadata) => metadata,
        Err(err) => {
            log::error!("failed to download video, URL: {url}, ERROR: {err}");
            addr.do_send(NotifyDownloadUpdate {
                result: DownloadUpdate::SingleFinished(Err((
                    info,
                    ErrorResponse {
                        error: format!("failed to download video with url: {url}"),
                    },
                ))),
            });
            return;
        }
    };

    addr.do_send(NotifyDownloadUpdate {
        result: DownloadUpdate::SingleFinished(Ok((info, metadata, identifier.to_path_with_ext()))),
    });
}

async fn download_youtube(
    identifier: &DownloadIdentifier,
    mut tx: sqlx::Transaction<'_, sqlx::Postgres>,
) -> anyhow::Result<AudioMetaData> {
    let metadata: AudioMetaData = get_video_metadata(identifier.url(), yt_api_key())
        .await?
        .into();

    let key = identifier.uid();
    sqlx::query!("INSERT INTO audio_metadata (identifier, name, author, duration, cover_art_url) values ($1, $2, $3, $4, $5)",
                    key,
                    metadata.name,
                    metadata.author,
                    metadata.duration,
                    metadata.cover_art_url
                )
                .execute(&mut *tx)
                .await?;

    let path = identifier.to_path();
    if let Err(err) = download_youtube_audio(identifier.url(), &path.to_string_lossy()) {
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

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn test_download_info_eq() {
        let info_1 = DownloadInfo::YoutubePlaylist {
            playlist_url: "123".to_owned(),
            video_urls: vec![],
        };
        let info_2 = DownloadInfo::YoutubePlaylist {
            playlist_url: "123".to_owned(),
            video_urls: vec!["ignored".to_owned()],
        };
        let info_3 = DownloadInfo::YoutubePlaylist {
            playlist_url: "13".to_owned(),
            video_urls: vec!["ignored".to_owned()],
        };

        let mut set: HashSet<DownloadInfo> = Default::default();

        assert_eq!(set.insert(info_1), true);
        assert_eq!(set.insert(info_2), false);
        assert_eq!(set.insert(info_3), true);
        assert_eq!(set.len(), 2)
    }
}
