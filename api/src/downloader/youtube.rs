use std::process::Command;

use actix::Recipient;
use anyhow::anyhow;
use sqlx::PgPool;

use crate::{
    audio_hosts::youtube::video::get_video_metadata, audio_playback::audio_item::AudioMetadata,
    yt_api_key, ErrorResponse,
};

use super::{
    actor::NotifyDownloadUpdate,
    download_identifier::{Identifier, YoutubeVideoUrl},
    info::DownloadInfo,
};

pub async fn process_single_youtube_video(
    url: &YoutubeVideoUrl<impl AsRef<str> + std::fmt::Display + std::fmt::Debug>,
    pool: &PgPool,
    addr: &Recipient<NotifyDownloadUpdate>,
) {
    let tx = pool.begin().await.unwrap();

    let info = DownloadInfo::yt_video(&url.0);

    let metadata = match download_and_store_youtube_audio_with_metadata(url, tx).await {
        Ok(metadata) => metadata,
        Err(err) => {
            log::error!(
                "failed to download video, URL: {url}, ERROR: {err}",
                url = url.0
            );
            addr.do_send(NotifyDownloadUpdate::SingleFinished(Err((
                info,
                ErrorResponse {
                    error: format!("failed to download video with url: {url}", url = url.0),
                },
            ))));
            return;
        }
    };

    addr.do_send(NotifyDownloadUpdate::SingleFinished(Ok((
        info,
        metadata,
        url.to_path_with_ext(),
    ))));
}

pub async fn download_and_store_youtube_audio_with_metadata(
    url: &YoutubeVideoUrl<impl AsRef<str> + std::fmt::Debug>,
    mut tx: sqlx::Transaction<'_, sqlx::Postgres>,
) -> anyhow::Result<AudioMetadata> {
    let metadata: AudioMetadata =
        AudioMetadata::from(get_video_metadata(url.0.as_ref(), yt_api_key()).await?);

    let key = url.uid();
    sqlx::query!("INSERT INTO audio_metadata (identifier, name, author, duration, cover_art_url) values ($1, $2, $3, $4, $5)",
                    key,
                    metadata.name.inner_as_ref(),
                    metadata.author.inner_as_ref(),
                    metadata.duration,
                    metadata.cover_art_url.inner_as_ref()
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

pub fn download_youtube_audio(url: &str, download_location: &str) -> anyhow::Result<()> {
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
