use crate::{utils::log_msg_received, ErrorResponse};
use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, Mutex},
    time::Duration,
};

use actix::{Actor, Context, Handler, Message, Recipient};
use actix_rt::Arbiter;
use anyhow::anyhow;
use serde::{Deserialize, Serialize};
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
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../app/src/api-types/")]
pub enum DownloadIdentifier {
    YouTube { url: String },
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct NotifyDownloadFinished {
    pub result: Result<DownloadIdentifier, (DownloadIdentifier, ErrorResponse)>,
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
                process_queue(queue.clone());
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
            .lock()
            .map_err(|err| ErrorResponse {
                error: format!("failed to add audio to download queue\nERROR: {err}"),
            })?
            .push_back(msg);
        Ok(())
    }
}

fn process_queue(queue: Arc<Mutex<VecDeque<DownloadAudioRequest>>>) {
    let mut queue = match queue.lock() {
        Ok(queue) => queue,
        Err(err) => {
            log::error!("download queue: errror during processing\nERROR: {err}");
            return;
        }
    };

    if let Some(req) = queue.pop_front() {
        let DownloadAudioRequest { addr, identifier } = req;
        log::info!("download for {identifier:?} has started");

        let path = identifier.to_path();

        // TODO: get metadata from YouTube API and return way to query file path and metadata from
        // db
        match &identifier {
            DownloadIdentifier::YouTube { url } => {
                if let Err(err) = download_youtube_audio(&url, &path.to_string_lossy().to_string())
                {
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

                addr.do_send(NotifyDownloadFinished {
                    result: Ok(identifier),
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
