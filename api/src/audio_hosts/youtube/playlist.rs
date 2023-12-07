use anyhow::anyhow;
use serde::Deserialize;

pub async fn get_playlist_video_urls(url: &str, api_key: &str) -> anyhow::Result<Vec<String>> {
    let Some(playlist_id) = extract_playlist_id(url) else {
        log::error!("failed to extract 'playlist id' from youtube playlist with url {url}");
        return Err(anyhow!("faild to download youtube playlist {url}"));
    };

    let api_url = format!("https://youtube.googleapis.com/youtube/v3/playlistItems?part=snippet&playlistId={playlist_id}&key={api_key}");
    let resp_text = reqwest::get(api_url).await?.text().await?;
    log::info!("{resp_text}");

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct YoutubePlaylistItems {
        items: Vec<YoutubePlaylistItem>,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct YoutubePlaylistItem {
        snippet: YoutubePlaylistItemSnippet,
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
        .map(|item| {
            format!(
                "https://www.youtube.com/watch?v={id}",
                id = item.snippet.resource_id.video_id
            )
        })
        .collect())
}

fn extract_playlist_id(url: &str) -> Option<&str> {
    url.split_once("playlist?list=").map(|s| s.1)
}
