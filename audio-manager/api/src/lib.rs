use actix::Addr;
use brain::brain_server::AudioBrain;
use serde::{Deserialize, Serialize};

pub mod commands;
pub mod streams;

pub mod audio;
pub mod brain;
pub mod downloader;
pub mod node;
pub mod utils;

pub static AUDIO_DIR: &str = "audio";

pub static AUDIO_SOURCES: [(&str, &str); 2] =
    [("Living Room", "living_room"), ("Office", "office")];

pub struct AppData {
    pub brain_addr: Addr<AudioBrain>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorResponse {
    error: String,
}
