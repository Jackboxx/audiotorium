use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::{
    audio_playback::audio_player::AudioInfo, downloader::actor::SerializableDownloadAudioRequest,
    node::node_server::SourceName,
};

pub mod restore_state_actor;

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct AppStateRecoveryInfo {
    download_info: DownloadStateInfo,
    audio_info: HashMap<SourceName, AudioInfo>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct DownloadStateInfo {
    queue: Vec<SerializableDownloadAudioRequest>,
}

#[cfg(test)]
mod tests {
    use crate::audio_playback::audio_player::PlaybackState;

    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_state_serialization() {
        let state = AppStateRecoveryInfo {
            audio_info: HashMap::from([(
                "test".into(),
                AudioInfo {
                    playback_state: PlaybackState::Paused,
                    current_queue_index: 3,
                    audio_progress: 0.43,
                    audio_volume: 0.23,
                },
            )]),
            download_info: DownloadStateInfo { queue: vec![] },
        };

        let bin = bincode::serialize(&state).unwrap();
        let decoded: AppStateRecoveryInfo = bincode::deserialize(&bin).unwrap();

        assert_eq!(
            state.audio_info.get("test").unwrap().current_queue_index,
            decoded.audio_info.get("test").unwrap().current_queue_index
        );
        assert_eq!(
            state.audio_info.get("test").unwrap().audio_volume,
            decoded.audio_info.get("test").unwrap().audio_volume
        );
        assert_eq!(
            state.audio_info.get("test").unwrap().audio_progress,
            decoded.audio_info.get("test").unwrap().audio_progress
        );
        assert_eq!(
            state.download_info.queue.len(),
            decoded.download_info.queue.len()
        );
    }
}
