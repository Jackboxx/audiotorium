use anyhow::anyhow;
use serde::Deserialize;

use crate::audio_playback::audio_item::AudioMetaData;

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
    pub duration_iso_8601: String,
}

impl From<YoutubeVideo> for AudioMetaData {
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

        AudioMetaData {
            name: Some(value.snippet.title),
            author: Some(value.snippet.channel_title),
            cover_art_url: Some(value.snippet.thumbnails.maxres.url),
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

pub async fn get_video_metadata(url: &str, api_key: &str) -> anyhow::Result<YoutubeVideo> {
    let Some(watch_id) = extract_watch_id(url) else {
        log::error!("failed to extract 'watch id' from youtube video with url {url}");
        return Err(anyhow!("faild to download youtube video {url}"));
    };

    let api_url =
        format!("https://www.googleapis.com/youtube/v3/videos?part=snippet,contentDetails&id={watch_id}&key={api_key}");

    let resp_text = reqwest::get(api_url).await?.text().await?;

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct YoutubeVideoItems {
        items: Vec<YoutubeVideo>,
    }

    let videos: YoutubeVideoItems = serde_json::from_str(&resp_text)?;
    let Some(video) = videos.items.into_iter().next() else {
        return Err(anyhow!("no youtube video found for id {watch_id}"));
    };

    Ok(video)
}

fn extract_watch_id(url: &str) -> Option<&str> {
    url.split_once("watch?v=").map(|s| s.1)
}