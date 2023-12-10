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
pub enum AudioKind {
    YoutubeVideo,
    YoutubePlaylist,
}

impl AudioKind {
    pub fn from_uid<T: AsRef<str> + std::fmt::Debug>(uid: &AudioUid<T>) -> Option<Self> {
        match uid {
            s if s.0.as_ref().starts_with(AudioKind::YoutubeVideo.prefix()) => {
                Some(AudioKind::YoutubeVideo)
            }
            s if s
                .0
                .as_ref()
                .starts_with(AudioKind::YoutubePlaylist.prefix()) =>
            {
                Some(AudioKind::YoutubePlaylist)
            }
            _ => None,
        }
    }

    fn prefix(&self) -> &str {
        match self {
            Self::YoutubeVideo => "youtube_audio_",
            Self::YoutubePlaylist => "youtube_playlist_audio_",
        }
    }
}

#[derive(Debug)]
pub struct AudioUid<T: AsRef<str> + std::fmt::Debug>(pub T);

impl<T: AsRef<str> + std::fmt::Debug> Identifier for AudioUid<T> {
    fn uid(&self) -> String {
        self.0.as_ref().to_string()
    }
}

#[derive(Debug)]
pub struct YoutubeVideoUrl<T: AsRef<str> + std::fmt::Debug>(pub T);

#[derive(Debug)]
pub struct YoutubePlaylistUrl<T: AsRef<str> + std::fmt::Debug>(pub T);

impl<T: AsRef<str> + std::fmt::Debug> Identifier for YoutubeVideoUrl<T> {
    fn uid(&self) -> String {
        let prefix = AudioKind::YoutubeVideo.prefix();
        let hex_url = hex::encode(self.0.as_ref());

        format!("{prefix}{hex_url}")
    }
}

impl<T: AsRef<str> + std::fmt::Debug> Identifier for YoutubePlaylistUrl<T> {
    fn uid(&self) -> String {
        let prefix = AudioKind::YoutubePlaylist.prefix();
        let hex_url = hex::encode(self.0.as_ref());

        format!("{prefix}{hex_url}")
    }
}

impl Clone for YoutubeVideoUrl<Arc<str>> {
    fn clone(&self) -> Self {
        YoutubeVideoUrl(Arc::clone(&self.0))
    }
}

impl Clone for YoutubePlaylistUrl<Arc<str>> {
    fn clone(&self) -> Self {
        YoutubePlaylistUrl(Arc::clone(&self.0))
    }
}
