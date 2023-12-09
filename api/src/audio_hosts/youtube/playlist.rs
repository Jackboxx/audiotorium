use anyhow::anyhow;
use serde::Deserialize;

use crate::audio_hosts::youtube::YoutubeStatus;

use super::YoutubeSnippet;

pub async fn get_playlist_metadata(url: &str, api_key: &str) -> anyhow::Result<YoutubeSnippet> {
    let Some(playlist_id) = extract_playlist_id(url) else {
        log::error!("failed to extract 'playlist id' from youtube playlist with url {url}");
        return Err(anyhow!("faild to download youtube playlist {url}"));
    };

    let api_url = format!("https://youtube.googleapis.com/youtube/v3/playlists?part=snippet&maxResults=1&id={playlist_id}&key={api_key}");
    let resp_text = reqwest::get(api_url).await?.text().await?;

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

    let playlists: YoutubePlaylistItems = serde_json::from_str(&resp_text)?;
    let Some(playlist) = playlists.items.into_iter().next() else {
        return Err(anyhow!("no youtube playlist found for id {playlist_id}"));
    };

    Ok(playlist.snippet)
}

pub async fn get_playlist_video_urls(url: &str, api_key: &str) -> anyhow::Result<Vec<String>> {
    let Some(playlist_id) = extract_playlist_id(url) else {
        log::error!("failed to extract 'playlist id' from youtube playlist with url {url}");
        return Err(anyhow!("faild to download youtube playlist {url}"));
    };

    let api_url = format!("https://youtube.googleapis.com/youtube/v3/playlistItems?part=snippet,status&maxResults=50&playlistId={playlist_id}&key={api_key}");
    let resp_text = reqwest::get(api_url).await?.text().await?;

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct YoutubePlaylistItems {
        items: Vec<YoutubePlaylistItem>,
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

    let playlist_items: YoutubePlaylistItems = serde_json::from_str(&resp_text)?;
    Ok(playlist_items
        .items
        .into_iter()
        .filter_map(|item| {
            is_public(&item.status).then_some(format!(
                "https://www.youtube.com/watch?v={id}",
                id = item.snippet.resource_id.video_id
            ))
        })
        .collect())
}

fn is_public(status: &YoutubeStatus) -> bool {
    status.privacy_status == "public"
}

fn extract_playlist_id(url: &str) -> Option<&str> {
    url.split_once("playlist?list=").map(|s| s.1)
}
