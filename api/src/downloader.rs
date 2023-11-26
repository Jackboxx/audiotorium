use crate::{
    audio::audio_item::AudioMetaData, db_pool, utils::log_msg_received, ErrorResponse, IntoErrResp,
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
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tokio::sync::Mutex;
use ts_rs::TS;

#[cfg(not(debug_assertions))]
const AUDIO_DIR: &str = "audio";

#[cfg(debug_assertions)]
const AUDIO_DIR: &str = "audio-dev";

pub struct AudioDownloader {
    download_thread: Arbiter,
    queue: Arc<Mutex<VecDeque<DownloadAudioRequest>>>,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "../app/src/api-types/")]
pub enum DownloadIdentifier {
    YouTube { url: String },
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct NotifyDownloadFinished {
    pub result: Result<(DownloadIdentifier, AudioMetaData), (DownloadIdentifier, ErrorResponse)>,
}

#[derive(Debug, Clone, Message)]
#[rtype(result = "Result<(), ErrorResponse>")]
pub struct DownloadAudioRequest {
    pub addr: Recipient<NotifyDownloadFinished>,
    pub identifier: DownloadIdentifier,
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
            Self::YouTube { url } => {
                let prefix = "youtube_audio_";
                let hex_url = hex::encode(url);

                format!("{prefix}{hex_url}")
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
    type Result = Result<(), ErrorResponse>;

    fn handle(&mut self, msg: DownloadAudioRequest, _ctx: &mut Self::Context) -> Self::Result {
        log_msg_received(&self, &msg);

        self.queue
            .try_lock()
            .into_err_resp("failed to add audio to download queue\nERROR:")?
            .push_back(msg);

        Ok(())
    }
}

async fn download_youtube(
    identifier: &DownloadIdentifier,
    url: &str,
    mut tx: sqlx::Transaction<'_, sqlx::Postgres>,
) -> anyhow::Result<AudioMetaData> {
    // TODO: get metadata from YouTube API and return way to query file path and metadata from
    // db
    let metadata = AudioMetaData {
        name: Some("test name".to_owned()),
        author: None,
        duration: Some(23434),
        cover_art_url: None,
    };

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
    if let Err(err) = download_youtube_audio(url, &path.to_string_lossy()) {
        if let Err(rollback_err) = tx.rollback().await {
            return Err(anyhow!("ERROR 1: {err}\nERROR 2: {rollback_err}"));
        }

        return Err(err);
    }

    tx.commit().await?;
    Ok(metadata)
}

async fn process_queue(queue: Arc<Mutex<VecDeque<DownloadAudioRequest>>>, pool: &PgPool) {
    let mut queue = queue.lock().await;

    if let Some(req) = queue.pop_front() {
        let DownloadAudioRequest { addr, identifier } = req;
        log::info!("download for {identifier:?} has started");

        match &identifier {
            DownloadIdentifier::YouTube { url } => {
                let tx = pool.begin().await.unwrap();
                let metadata = match download_youtube(&identifier, url, tx).await {
                    Ok(metadata) => metadata,
                    Err(err) => {
                        log::error!("failed to download video, URL: {url}, ERROR: {err}");
                        addr.do_send(NotifyDownloadFinished {
                            result: Err((
                                identifier.clone(),
                                ErrorResponse {
                                    error: format!(
                                        "failed to download video with url: {url}, ERROR: {err}"
                                    ),
                                },
                            )),
                        });
                        return;
                    }
                };

                addr.do_send(NotifyDownloadFinished {
                    result: Ok((identifier, metadata)),
                });
            }
        }
    }
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
        dbg!(out);
        return Err(anyhow!(""));
    }

    Ok(())
}
