use crate::{
    audio::audio_item::{AudioMetaData, AudioPlayerQueueItem},
    utils::type_as_str,
    ErrorResponse,
};
use std::{path::PathBuf, process::Command};

use actix::{Actor, Context, Handler, Message, Recipient};
use anyhow::anyhow;

#[derive(Default)]
pub struct AudioDownloader;

#[derive(Debug, Clone, Message)]
#[rtype(result = "()")]
pub struct NotifyDownloadFinished {
    pub result: Result<DownloadAudioResponse, ErrorResponse>,
}

#[derive(Debug, Clone, Message)]
#[rtype(result = "()")]
pub struct DownloadAudio {
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
    }
}

impl Handler<DownloadAudio> for AudioDownloader {
    type Result = ();

    fn handle(&mut self, msg: DownloadAudio, _ctx: &mut Self::Context) -> Self::Result {
        log::info!(
            "{} received message {}\ncontent: {msg:?}",
            type_as_str(&self),
            type_as_str(&msg)
        );

        let DownloadAudio { addr, path, url } = msg;

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
