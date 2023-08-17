use crate::ErrorResponse;
use std::{process::Command, path::PathBuf};

use actix::{Actor, Context, Handler, Message, Recipient};
use anyhow::anyhow;
use log::{info, error};

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
    pub path: PathBuf,
}

impl Actor for AudioDownloader {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        info!("stared new 'AudioDownloader', CONTEXT: {ctx:?}");
    }
}

impl Handler<DownloadAudio> for AudioDownloader {
    type Result = (); 

    fn handle(&mut self, msg: DownloadAudio, _ctx: &mut Self::Context) -> Self::Result {
        let DownloadAudio { addr, path, url }  = msg;

        let Some(str_path) = path.to_str() else { 
            error!("path {path:?} can't be converted to a string");
            addr.do_send(NotifyDownloadFinished { result: Err(ErrorResponse { error: format!("failed to construct valid path") }) });
            return;
        };

        if let Err(err) = download_audio(&url, str_path) {
            error!("failed to download video, URL: {url}, ERROR: {err}");
            addr.do_send(NotifyDownloadFinished { result: Err(ErrorResponse { error: format!("failed to download video with url: {url}, ERROR: {err}") })});
            return;
        }

        let path_with_ext = path.with_extension("mp3");
        addr.do_send(NotifyDownloadFinished { result: Ok(DownloadAudioResponse { path: path_with_ext }) });
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
