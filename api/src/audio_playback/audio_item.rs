use std::path::PathBuf;

use creek::{OpenError, ReadDiskStream, SymphoniaDecoder};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, TS)]
#[ts(export, export_to = "../app/src/api-types/")]
pub struct AudioMetaData {
    pub name: Option<String>,
    pub author: Option<String>,
    pub duration: Option<i64>,
    pub cover_art_url: Option<String>,
}

pub trait AudioDataLocator: Send {
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
