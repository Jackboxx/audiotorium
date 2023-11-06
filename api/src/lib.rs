use actix::Addr;
use brain::brain_server::AudioBrain;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use ts_rs::TS;

pub mod commands;
pub mod streams;

pub mod audio;
pub mod brain;
pub mod downloader;
pub mod message_send_handler;
pub mod node;
pub mod utils;

#[cfg(test)]
pub mod tests_utils;

pub struct AppData {
    db_pool: SqlitePool,
    brain_addr: Addr<AudioBrain>,
}

impl AppData {
    pub fn new(db_pool: SqlitePool, brain_addr: Addr<AudioBrain>) -> Self {
        Self {
            db_pool,
            brain_addr,
        }
    }
    pub fn db_pool(&self) -> &SqlitePool {
        &self.db_pool
    }

    pub fn brain_addr(&self) -> &Addr<AudioBrain> {
        &self.brain_addr
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../app/src/api-types/")]
pub struct ErrorResponse {
    error: String,
}
