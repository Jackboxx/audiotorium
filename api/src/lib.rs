use actix::Addr;
use brain::brain_server::AudioBrain;
use serde::{Deserialize, Serialize};
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
    pub brain_addr: Addr<AudioBrain>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../app/src/api-types/")]
pub struct ErrorResponse {
    error: String,
}
