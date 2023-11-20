use std::sync::OnceLock;

use actix::Addr;
use brain::brain_server::AudioBrain;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use ts_rs::TS;

pub mod commands;
pub mod streams;

pub mod audio;
pub mod brain;
pub mod downloader;
pub mod message_send_handler;
pub mod node;
pub mod utils;

pub static POOL: OnceLock<PgPool> = OnceLock::new(); // set on server start

#[cfg(test)]
pub mod tests_utils;

pub struct AppData {
    brain_addr: Addr<AudioBrain>,
}

impl AppData {
    pub fn new(brain_addr: Addr<AudioBrain>) -> Self {
        Self { brain_addr }
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
