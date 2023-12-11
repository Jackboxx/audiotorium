use std::sync::OnceLock;

use actix::Addr;
use brain::brain_server::AudioBrain;
use sqlx::PgPool;

pub mod commands;
pub mod streams;

pub mod audio_hosts;
pub mod audio_playback;
pub mod brain;
pub mod database;
pub mod downloader;
pub mod error;
pub mod message_send_handler;
pub mod node;
pub mod opt_arc;
pub mod rest_data_access;
pub mod utils;

pub static POOL: OnceLock<PgPool> = OnceLock::new(); // set on server start
pub static YOUTUBE_API_KEY: OnceLock<String> = OnceLock::new(); // set on server start

pub fn db_pool<'a>() -> &'a PgPool {
    POOL.get().expect("pool should be set at server start")
}

pub fn yt_api_key<'a>() -> &'a str {
    YOUTUBE_API_KEY
        .get()
        .expect("youtube api key should be set at server start")
}

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
