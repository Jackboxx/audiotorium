use std::{path::PathBuf, sync::Arc};

use creek::{OpenError, ReadDiskStream, SymphoniaDecoder};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use ts_rs::TS;

use crate::{downloader::download_identifier::ItemUid, opt_arc::OptionArcStr};

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, TS)]
#[ts(export, export_to = "../app/src/api-types/")]
pub struct AudioMetadata {
    pub name: OptionArcStr,
    pub author: OptionArcStr,
    pub duration: Option<i64>,
    pub cover_art_url: OptionArcStr,
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
    pub identifier: ItemUid<Arc<str>>,
    pub metadata: AudioMetadata,
    pub locator: ADL,
}
