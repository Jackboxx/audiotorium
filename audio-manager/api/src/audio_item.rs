use std::{path::PathBuf, time::Duration};

use creek::{OpenError, ReadDiskStream, SymphoniaDecoder};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioMetaData {
    pub name: String,
    pub author: Option<String>,
    pub duration: Option<Duration>,
    pub thumbnail_url: Option<String>,
}

pub trait AudioDataLocator {
    fn load_audio_data(&self) -> Result<ReadDiskStream<SymphoniaDecoder>, OpenError>;
}

impl AudioDataLocator for PathBuf {
    fn load_audio_data(&self) -> Result<ReadDiskStream<SymphoniaDecoder>, OpenError> {
        ReadDiskStream::<SymphoniaDecoder>::new(self, 0, Default::default())
    }
}

#[derive(Debug, Clone)]
pub struct AudioPlayerQueueItem<ADL: AudioDataLocator> {
    pub metadata: AudioMetaData,
    pub locator: ADL,
}
