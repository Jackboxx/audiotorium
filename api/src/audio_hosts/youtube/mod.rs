use std::sync::Arc;

use serde::Deserialize;

use crate::error::{AppError, AppErrorKind, IntoAppError};

pub mod playlist;
pub mod video;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct YoutubeSnippet {
    pub title: Arc<str>,
    pub channel_title: Arc<str>,
    pub thumbnails: YoutubeMaxResThumbnail,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct YoutubeStatus {
    pub privacy_status: Arc<str>,
}

#[derive(Debug, Deserialize)]
pub struct YoutubeMaxResThumbnail {
    pub maxres: YoutubeThumbnail,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct YoutubeThumbnail {
    pub url: Arc<str>,
    pub width: u64,
    pub height: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum YoutubeContentType {
    Video,
    Playlist,
    Invalid,
}

pub fn youtube_content_type<'a>(value: impl Into<&'a str>) -> YoutubeContentType {
    let value = value.into();

    fn yt_type(value: &str) -> YoutubeContentType {
        match value {
            s if s.starts_with("https://www.youtube.com/watch?v=") => YoutubeContentType::Video,
            s if s.starts_with("https://www.youtube.com/playlist?list=") => {
                YoutubeContentType::Playlist
            }
            _ => YoutubeContentType::Invalid,
        }
    }

    yt_type(value)
}

async fn get_api_data(url: &str) -> Result<String, AppError> {
    reqwest::get(url)
        .await
        .into_app_err(
            "failed to fetch youtube playlist metadata",
            AppErrorKind::Api,
            &[&format!("URL: {url}")],
        )?
        .text()
        .await
        .into_app_err(
            "failed to fetch youtube playlist metadata",
            AppErrorKind::Api,
            &[&format!("URL: {url}")],
        )
}

fn parse_api_data<'a, T: Deserialize<'a>>(body: &'a str, url: &'a str) -> Result<T, AppError> {
    serde_json::from_str(body).into_app_err(
        "failed to parse youtube playlist metadata",
        AppErrorKind::Api,
        &[&format!("URL: {url}"), &format!("RESPONSE_TEXT: {body}")],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_youtube_content_type() {
        assert_eq!(
            youtube_content_type("https://www.youtube.com/watch?v=HYd9B6YvIHM"),
            YoutubeContentType::Video
        );

        assert_eq!(
            youtube_content_type("https://www.youtube.com/watch?v=JogLvpzvn4Q&list=PLGK-2zLAFymBMRyVJCmS2jg8x-P2I4Y-J&index=2"),
            YoutubeContentType::Video
        );

        assert_eq!(
            youtube_content_type(
                "https://www.youtube.com/playlist?list=PLGK-2zLAFymBMRyVJCmS2jg8x-P2I4Y-J"
            ),
            YoutubeContentType::Playlist
        );

        assert_eq!(
            youtube_content_type("https://www.yt.com/watch?v=HYd9B6YvIHM"),
            YoutubeContentType::Invalid
        );

        assert_eq!(
            youtube_content_type("https://www.youtube.com/short?s=HYd9B6YvIHM"),
            YoutubeContentType::Invalid
        );
    }
}
