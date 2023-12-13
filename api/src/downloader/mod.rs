use std::sync::Arc;

use serde::{Deserialize, Serialize};

use self::download_identifier::{YoutubePlaylistUrl, YoutubeVideoUrl};

pub mod actor;
pub mod download_identifier;
pub mod info;
mod youtube;

#[cfg(not(debug_assertions))]
pub const AUDIO_DIR: &str = "audio";

#[cfg(debug_assertions)]
pub const AUDIO_DIR: &str = "audio-dev";

#[derive(Debug, Deserialize, Serialize)]
pub enum DownloadRequiredInformation {
    StoredLocally { uid: Arc<str> },
    YoutubeVideo { url: YoutubeVideoUrl<Arc<str>> },
    YoutubePlaylist(YoutubePlaylistDownloadInfo),
}

#[derive(Debug, Deserialize, Serialize)]
pub struct YoutubePlaylistDownloadInfo {
    pub playlist_url: YoutubePlaylistUrl<Arc<str>>,
    pub video_urls: Arc<[Arc<str>]>,
}
