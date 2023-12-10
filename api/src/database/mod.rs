use std::sync::Arc;

use serde::{Deserialize, Serialize};
use sqlx::prelude::FromRow;
use ts_rs::TS;

pub mod fetch_data;
pub mod store_data;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, TS)]
#[ts(export, export_to = "../app/src/api-types/")]
pub struct PlaylistMetadata {
    pub name: Option<Arc<str>>,
    pub author: Option<Arc<str>>,
    pub cover_art_url: Option<Arc<str>>,
}
