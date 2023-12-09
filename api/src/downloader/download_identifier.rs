use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use super::AUDIO_DIR;

pub trait Identifier {
    fn uid(&self) -> String;
    fn to_path(&self) -> PathBuf {
        Path::new(AUDIO_DIR).join(self.uid())
    }

    fn to_path_with_ext(&self) -> PathBuf {
        self.to_path().with_extension("wav")
    }
}

#[derive(Debug)]
pub enum DownloadRequiredInformation {
    YoutubeVideo { url: YoutubeVideoUrl<Arc<str>> },
    YoutubePlaylist(YoutubePlaylistDownloadInfo),
}

#[derive(Debug)]
pub enum DownloadKind {
    YoutubeVideo,
    YoutubePlaylist,
}

#[derive(Debug)]
pub struct YoutubePlaylistDownloadInfo {
    pub playlist_url: YoutubePlaylistUrl<Arc<str>>,
    pub video_urls: Vec<Arc<str>>,
}

// impl DownloadKind {
//     fn from<T: AsRef<str>, V: Borrow<DownloadRequiredInformation<T>>>(ident: V) -> Self {
//         match ident.borrow() {
//             DownloadRequiredInformation::YoutubeVideo { .. } => DownloadKind::YoutubeVideo,
//             DownloadRequiredInformation::YoutubePlaylist { .. } => DownloadKind::YoutubePlaylist,
//         }
//     }
// }
//
impl DownloadKind {
    fn prefix(&self) -> &str {
        match self {
            Self::YoutubeVideo => "youtube_audio_",
            Self::YoutubePlaylist => "youtube_playlist_audio_",
        }
    }
}

#[derive(Debug)]
pub struct YoutubeVideoUrl<T: AsRef<str> + std::fmt::Debug>(pub T);

#[derive(Debug)]
pub struct YoutubePlaylistUrl<T: AsRef<str> + std::fmt::Debug>(pub T);

impl<T: AsRef<str> + std::fmt::Debug> Identifier for YoutubeVideoUrl<T> {
    fn uid(&self) -> String {
        let prefix = DownloadKind::YoutubeVideo.prefix();
        let hex_url = hex::encode(self.0.as_ref());

        format!("{prefix}{hex_url}")
    }
}

impl<T: AsRef<str> + std::fmt::Debug> Identifier for YoutubePlaylistUrl<T> {
    fn uid(&self) -> String {
        let prefix = DownloadKind::YoutubePlaylist.prefix();
        let hex_url = hex::encode(self.0.as_ref());

        format!("{prefix}{hex_url}")
    }
}
