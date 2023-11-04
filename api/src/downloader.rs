use crate::{
    audio::audio_item::{AudioMetaData, AudioPlayerQueueItem},
    utils::log_msg_received,
    ErrorResponse,
};
use std::{
    collections::VecDeque,
    path::PathBuf,
    process::Command,
    sync::{Arc, Mutex},
    time::Duration,
};

use actix::{Actor, Context, Handler, Message, Recipient};
use anyhow::anyhow;

#[derive(Default)]
pub struct AudioDownloader {
    queue: Arc<Mutex<VecDeque<DownloadAudioRequest>>>,
}

#[derive(Debug, Clone, Message)]
#[rtype(result = "()")]
pub struct NotifyDownloadFinished {
    pub result: Result<DownloadAudioResponse, ErrorResponse>,
}

#[derive(Debug, Clone, Message)]
#[rtype(result = "Result<(), ErrorResponse>")]
pub struct DownloadAudioRequest {
    pub addr: Recipient<NotifyDownloadFinished>,
    pub path: PathBuf,
    pub url: String,
}

#[derive(Debug, Clone, Message)]
#[rtype(result = "()")]
pub struct DownloadAudioResponse {
    pub item: AudioPlayerQueueItem<PathBuf>,
}

impl Actor for AudioDownloader {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        log::info!("stared new 'AudioDownloader', CONTEXT: {ctx:?}");

        let queue = self.queue.clone();

        actix_rt::spawn(async move {
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
        let DownloadAudioRequest { addr, path, url } = req;
        log::info!("download for {url:?} has started");

        let Some(str_path) = path.to_str() else {
            log::error!("path {path:?} can't be converted to a string");
            addr.do_send(NotifyDownloadFinished {
                result: Err(ErrorResponse {
                    error: "failed to construct valid path".to_owned(),
                }),
            });
            return;
        };

        if let Err(err) = download_audio(&url, str_path) {
            log::error!("failed to download video, URL: {url}, ERROR: {err}");
            addr.do_send(NotifyDownloadFinished {
                result: Err(ErrorResponse {
                    error: format!("failed to download video with url: {url}, ERROR: {err}"),
                }),
            });
            return;
        }

        let path_with_ext = path.with_extension("mp3");
        let item = AudioPlayerQueueItem {
            metadata: AudioMetaData {
                name: path
                    .file_stem()
                    .map(|os_str| os_str.to_string_lossy().to_string())
                    .unwrap_or(String::new()),
                author: None,
                duration: None,
                thumbnail_url: None,
            },
            locator: path_with_ext,
        };

        addr.do_send(NotifyDownloadFinished {
            result: Ok(DownloadAudioResponse { item }),
        });
    }
}

fn download_audio(url: &str, download_location: &str) -> anyhow::Result<()> {
    let out = Command::new("yt-dlp")
        .args([
            "-f",
            "bestaudio",
            "-x",
            "--audio-format",
            "mp3",
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
