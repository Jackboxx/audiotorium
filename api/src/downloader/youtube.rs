use std::process::Command;

use actix::Recipient;
use sqlx::PgPool;

use crate::{
    audio_hosts::youtube::video::get_video_metadata,
    audio_playback::audio_item::AudioMetadata,
    database::fetch_data::get_audio_metadata_from_db,
    error::{AppError, AppErrorKind, IntoAppError},
    yt_api_key,
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
    let info = DownloadInfo::yt_video(&url.0);

    let tx = match pool.begin().await.into_app_err(
        "failed to start transaction",
        AppErrorKind::Database,
        &[],
    ) {
        Ok(tx) => tx,
        Err(err) => {
            addr.do_send(NotifyDownloadUpdate::SingleFinished(Err((info, err))));
            return;
        }
    };

    let metadata = match download_and_store_youtube_audio_with_metadata(url, tx).await {
        Ok(metadata) => metadata,
        Err(err) => {
            addr.do_send(NotifyDownloadUpdate::SingleFinished(Err((info, err))));
            return;
        }
    };

    let uid = url.uid();
    addr.do_send(NotifyDownloadUpdate::SingleFinished(Ok((
        info, metadata, uid,
    ))));
}

pub async fn download_and_store_youtube_audio_with_metadata(
    url: &YoutubeVideoUrl<impl AsRef<str> + std::fmt::Debug>,
    mut tx: sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<AudioMetadata, AppError> {
    let uid = url.uid();
    if let Some(metadata) = get_audio_metadata_from_db(&uid).await? {
        return Ok(metadata);
    }

    let metadata: AudioMetadata =
        AudioMetadata::from(get_video_metadata(url.0.as_ref(), yt_api_key()).await?);

    let key = uid.0.as_ref();
    sqlx::query!("INSERT INTO audio_metadata (identifier, name, author, duration, cover_art_url) values ($1, $2, $3, $4, $5)",
                    key,
                    metadata.name.inner_as_ref(),
                    metadata.author.inner_as_ref(),
                    metadata.duration,
                    metadata.cover_art_url.inner_as_ref()
                )
                .execute(&mut *tx)
                .await.into_app_err("failed to store audio metadata", AppErrorKind::Database, 
                                    &[&format!("UID: {key}")]
                                    )?;

    let path = url.to_path_with_ext();
    download_youtube_audio(url.0.as_ref(), &path.to_string_lossy())?;

    tx.commit()
        .await
        .into_app_err("failed to commit transaction", AppErrorKind::Database, &[])?;

    Ok(metadata)
}

pub fn download_youtube_audio(url: &str, download_location: &str) -> Result<(), AppError> {
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
        .output()
        .into_app_err(
            "failed to download youtube video",
            AppErrorKind::Download,
            &[&format!("URL: {url}")],
        )?;

    if out.status.code().unwrap_or(1) != 0 {
        return Err(AppError::new(
            AppErrorKind::Download,
            "failed to download youtube video",
            &["failed to parse stderr of 'yt-dlp' command"],
        ));
    }

    Ok(())
}
