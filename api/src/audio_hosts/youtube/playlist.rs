use std::sync::Arc;

use serde::Deserialize;

use crate::{
    audio_hosts::youtube::{get_api_data, parse_api_data, YoutubeStatus},
    error::{AppError, AppErrorKind},
};

use super::YoutubeSnippet;

pub async fn get_playlist_metadata(url: &str, api_key: &str) -> Result<YoutubeSnippet, AppError> {
    let Some(playlist_id) = extract_playlist_id(url) else {
        return Err(AppError::new(
            AppErrorKind::Api,
            "failed to get 'playlist id' from youtube playlist url",
            &[&format!("URL: {url}")],
        ));
    };

    let api_url = format!("https://youtube.googleapis.com/youtube/v3/playlists?part=snippet&maxResults=1&id={playlist_id}&key={api_key}");
    let resp_text = get_api_data(&api_url).await?;

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct YoutubePlaylistItems {
        items: Vec<YoutubePlaylist>,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct YoutubePlaylist {
        snippet: YoutubeSnippet,
    }

    let playlists: YoutubePlaylistItems = parse_api_data(&resp_text, &api_url)?;

    let Some(playlist) = playlists.items.into_iter().next() else {
        return Err(AppError::new(
            AppErrorKind::Api,
            "failed to find youtube playlist",
            &[&format!("URL: {url}")],
        ));
    };

    Ok(playlist.snippet)
}

pub async fn get_playlist_video_urls(
    url: &str,
    api_key: &str,
) -> Result<Arc<[Arc<str>]>, AppError> {
    let Some(playlist_id) = extract_playlist_id(url) else {
        return Err(AppError::new(
            AppErrorKind::Api,
            "faild to download youtube playlist content",
            &[&format!("URL: {url}")],
        ));
    };

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct YoutubePlaylistItems {
        items: Vec<YoutubePlaylistItem>,
        next_page_token: Option<String>,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct YoutubePlaylistItem {
        snippet: YoutubePlaylistItemSnippet,
        status: YoutubeStatus,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct YoutubePlaylistItemSnippet {
        resource_id: YoutubePlaylistItemResourceId,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct YoutubePlaylistItemResourceId {
        video_id: String,
    }

    let base_api_url = format!("https://youtube.googleapis.com/youtube/v3/playlistItems?part=snippet,status&maxResults=50&playlistId={playlist_id}&key={api_key}");
    let resp_text = get_api_data(&base_api_url).await?;

    let mut response: YoutubePlaylistItems = parse_api_data(&resp_text, &base_api_url)?;
    let mut playlist_items = response.items;

    while let Some(token) = response.next_page_token {
        let api_url = format!("{base_api_url}&pageToken={token}");
        let resp_text = get_api_data(&api_url).await?;

        response = parse_api_data(&resp_text, &api_url)?;
        playlist_items.append(&mut response.items);
    }

    Ok(playlist_items
        .into_iter()
        .filter_map(|item| {
            is_public(&item.status).then_some(
                format!(
                    "https://www.youtube.com/watch?v={id}",
                    id = item.snippet.resource_id.video_id
                )
                .into(),
            )
        })
        .collect())
}

fn is_public(status: &YoutubeStatus) -> bool {
    status.privacy_status.as_ref() == "public"
}

fn extract_playlist_id(url: &str) -> Option<&str> {
    url.split_once("playlist?list=").map(|s| s.1)
}
