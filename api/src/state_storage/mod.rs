use serde::{Deserialize, Serialize};

use crate::{audio_playback::audio_player::AudioInfo, downloader::DownloadRequiredInformation};

#[derive(Debug, Deserialize, Serialize)]
pub struct AppStateInfo {
    download_info: DownloadStateInfo,
    audio_info: AudioInfo,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DownloadStateInfo {
    // queue: Vec<DownloadRequiredInformation>,
    queue: Vec<()>,
}

#[cfg(test)]
mod tests {
    use crate::audio_playback::audio_player::PlaybackState;

    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_state_serialization() {
        let state = AppStateInfo {
            audio_info: AudioInfo {
                playback_state: PlaybackState::Paused,
                current_queue_index: 3,
                audio_progress: 0.43,
                audio_volume: 0.23,
            },
            download_info: DownloadStateInfo { queue: vec![] },
        };

        let bin = bincode::serialize(&state).unwrap();
        let decoded: AppStateInfo = bincode::deserialize(&bin).unwrap();

        assert_eq!(
            state.audio_info.current_queue_index,
            decoded.audio_info.current_queue_index
        );
        assert_eq!(
            state.audio_info.audio_volume,
            decoded.audio_info.audio_volume
        );
        assert_eq!(
            state.audio_info.audio_progress,
            decoded.audio_info.audio_progress
        );
        assert_eq!(
            state.download_info.queue.len(),
            decoded.download_info.queue.len()
        );
    }
}
