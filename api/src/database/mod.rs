use serde::{Deserialize, Serialize};
use sqlx::prelude::FromRow;
use ts_rs::TS;

use crate::opt_arc::OptionArcStr;

pub mod fetch_data;
pub mod store_data;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, TS)]
#[ts(export, export_to = "../app/src/api-types/")]
pub struct PlaylistMetadata {
    pub name: OptionArcStr,
    pub author: OptionArcStr,
    pub cover_art_url: OptionArcStr,
}
