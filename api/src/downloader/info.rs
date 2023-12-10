use std::{borrow::Borrow, sync::Arc};

use serde::Serialize;
use ts_rs::TS;

use super::{DownloadRequiredInformation, YoutubePlaylistDownloadInfo};

#[derive(Debug, Clone, Eq, Serialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "../app/src/api-types/")]
pub enum DownloadInfo {
    YoutubeVideo {
        url: Arc<str>,
    },
    YoutubePlaylist {
        playlist_url: Arc<str>,
        #[ts(type = "Array<string>")]
        video_urls: Vec<Arc<str>>,
    },
}

impl std::hash::Hash for DownloadInfo {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Self::YoutubeVideo { url } => url.hash(state),
            Self::YoutubePlaylist { playlist_url, .. } => playlist_url.hash(state),
        };
    }
}

impl PartialEq for DownloadInfo {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (DownloadInfo::YoutubeVideo { url }, DownloadInfo::YoutubeVideo { url: url_other }) => {
                url.eq(url_other)
            }
            (
                DownloadInfo::YoutubePlaylist { playlist_url, .. },
                DownloadInfo::YoutubePlaylist {
                    playlist_url: playlist_url_other,
                    ..
                },
            ) => playlist_url.eq(playlist_url_other),
            _ => false,
        }
    }
}

impl DownloadInfo {
    pub fn yt_video_from_arc(video_url: &Arc<str>) -> Self {
        DownloadInfo::YoutubeVideo {
            url: Arc::clone(video_url),
        }
    }

    pub fn yt_video(video_url: impl AsRef<str>) -> Self {
        DownloadInfo::YoutubeVideo {
            url: video_url.as_ref().into(),
        }
    }

    pub fn yt_playlist_from_arc(playlist_url: &Arc<str>, video_urls: &[Arc<str>]) -> Self {
        DownloadInfo::YoutubePlaylist {
            playlist_url: Arc::clone(playlist_url),
            video_urls: video_urls.iter().map(|str| Arc::clone(str)).collect(),
        }
    }

    pub fn yt_playlist(playlist_url: impl AsRef<str>, video_urls: &[impl AsRef<str>]) -> Self {
        DownloadInfo::YoutubePlaylist {
            playlist_url: playlist_url.as_ref().into(),
            video_urls: video_urls.iter().map(|str| str.as_ref().into()).collect(),
        }
    }
}

impl<T: Borrow<DownloadRequiredInformation>> From<T> for DownloadInfo {
    fn from(value: T) -> Self {
        match value.borrow() {
            DownloadRequiredInformation::YoutubeVideo { url } => DownloadInfo::YoutubeVideo {
                url: Arc::clone(&url.0),
            },
            DownloadRequiredInformation::YoutubePlaylist(YoutubePlaylistDownloadInfo {
                playlist_url,
                video_urls,
            }) => DownloadInfo::YoutubePlaylist {
                playlist_url: Arc::clone(&playlist_url.0),
                video_urls: video_urls.into_iter().map(|str| Arc::clone(str)).collect(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn test_download_info_eq() {
        let info_1 = DownloadInfo::YoutubePlaylist {
            playlist_url: "123".into(),
            video_urls: vec![],
        };
        let info_2 = DownloadInfo::YoutubePlaylist {
            playlist_url: "123".into(),
            video_urls: vec!["ignored".into()],
        };
        let info_3 = DownloadInfo::YoutubePlaylist {
            playlist_url: "13".into(),
            video_urls: vec!["ignored".into()],
        };

        let mut set: HashSet<DownloadInfo> = Default::default();

        assert_eq!(set.insert(info_1), true);
        assert_eq!(set.insert(info_2), false);
        assert_eq!(set.insert(info_3), true);
        assert_eq!(set.len(), 2)
    }
}
