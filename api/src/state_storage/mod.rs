use std::{collections::HashMap, path::PathBuf, sync::Arc};

use serde::{Deserialize, Serialize};

use crate::{
    audio_playback::{audio_item::AudioPlayerQueueItem, audio_player::PlaybackState},
    database::fetch_data::get_audio_metadata_from_db,
    downloader::{
        actor::SerializableDownloadAudioRequest,
        download_identifier::{Identifier, ItemUid},
    },
    node::node_server::SourceName,
};

pub mod restore_state_actor;

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct AppStateRecoveryInfo {
    pub download_info: DownloadStateInfo,
    pub audio_info: HashMap<SourceName, AudioStateInfo>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AudioStateInfo {
    pub playback_state: PlaybackState,
    pub current_queue_index: usize,
    pub audio_progress: f64,
    pub audio_volume: f32,
    pub queue: Vec<ItemUid<Arc<str>>>,

    #[serde(skip_serializing, skip_deserializing)]
    pub restored_queue: Vec<AudioPlayerQueueItem<PathBuf>>,
}

impl Default for AudioStateInfo {
    fn default() -> Self {
        Self {
            audio_volume: 1.0,
            playback_state: Default::default(),
            current_queue_index: Default::default(),
            audio_progress: Default::default(),
            queue: Default::default(),
            restored_queue: Default::default(),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct DownloadStateInfo {
    pub queue: Vec<SerializableDownloadAudioRequest>,
}

impl AudioStateInfo {
    async fn restore_queue(&mut self) {
        let mut queue = Vec::with_capacity(self.queue.len());

        for uid in self.queue.iter() {
            if let Ok(Some(metadata)) = get_audio_metadata_from_db(&uid).await {
                let path = uid.to_path_with_ext();

                queue.push(AudioPlayerQueueItem {
                    identifier: uid.clone(),
                    locator: path,
                    metadata,
                })
            }
        }

        self.restored_queue = queue;
    }
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
                AudioStateInfo {
                    playback_state: PlaybackState::Paused,
                    current_queue_index: 3,
                    audio_progress: 0.43,
                    audio_volume: 0.23,
                    queue: vec![ItemUid("uid".into())],
                    restored_queue: vec![],
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
