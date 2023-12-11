use std::sync::Arc;

use serde::Deserialize;

use crate::{
    audio_hosts::youtube::{get_api_data, parse_api_data},
    audio_playback::audio_item::AudioMetadata,
    error::{AppError, AppErrorKind},
};

use super::YoutubeSnippet;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct YoutubeVideo {
    pub snippet: YoutubeSnippet,
    pub content_details: YoutubeVideoContentDetails,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct YoutubeVideoContentDetails {
    #[serde(rename = "duration")]
    pub duration_iso_8601: Arc<str>,
}

impl From<YoutubeVideo> for AudioMetadata {
    fn from(value: YoutubeVideo) -> Self {
        let duration = match value.content_details.duration() {
            Some(duration) => match duration.try_into() {
                Ok(duration) => Some(duration),
                Err(err) => {
                    log::error!("failed to convert duration {err}");
                    None
                }
            },
            None => None,
        };

        AudioMetadata {
            name: Some(value.snippet.title).into(),
            author: Some(value.snippet.channel_title).into(),
            cover_art_url: Some(value.snippet.thumbnails.maxres.url).into(),
            duration,
        }
    }
}

impl YoutubeVideoContentDetails {
    fn duration(&self) -> Option<u128> {
        match parse_duration::parse(&self.duration_iso_8601.replace("M", "m"))
            .map(|t| t.as_millis())
        {
            Ok(t) => Some(t),
            Err(err) => {
                log::error!("failed to parse duration {err}");
                None
            }
        }
    }
}

pub async fn get_video_metadata(url: &str, api_key: &str) -> Result<YoutubeVideo, AppError> {
    let Some(watch_id) = extract_watch_id(url) else {
        return Err(AppError::new(
            AppErrorKind::Download,
            "failed to get 'watch id' from youtube video url",
            &[&format!("URL: {url}")],
        ));
    };

    let api_url =
        format!("https://www.googleapis.com/youtube/v3/videos?part=snippet,contentDetails&id={watch_id}&key={api_key}");

    let resp_text = get_api_data(&api_url).await?;

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct YoutubeVideoItems {
        items: Vec<YoutubeVideo>,
    }

    let videos: YoutubeVideoItems = parse_api_data(&resp_text, &api_url)?;
    let Some(video) = videos.items.into_iter().next() else {
        return Err(AppError::new(
            AppErrorKind::Download,
            "failed to find youtube video",
            &[&format!("URL: {url}")],
        ));
    };

    Ok(video)
}

fn extract_watch_id(url: &str) -> Option<&str> {
    url.split_once("watch?v=").map(|s| s.1)
}
