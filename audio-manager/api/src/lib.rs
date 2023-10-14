use actix::Addr;
use brain::brain_server::AudioBrain;
use serde::{Deserialize, Serialize};

pub mod commands;
pub mod streams;

pub mod audio;
pub mod brain;
pub mod downloader;
pub mod message_handler;
pub mod node;
pub mod utils;

#[cfg(test)]
pub mod tests_utils;

pub static AUDIO_DIR: &str = "audio";

pub struct AppData {
    pub brain_addr: Addr<AudioBrain>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorResponse {
    error: String,
}
