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
pub mod path;
pub mod rest_data_access;
pub mod state_storage;
pub mod utils;

pub static POOL: OnceLock<PgPool> = OnceLock::new(); // set on server start
pub static YOUTUBE_API_KEY: OnceLock<String> = OnceLock::new(); // set on server start

pub static BRAIN_ADDR: OnceLock<Addr<AudioBrain>> = OnceLock::new(); // set on server start

pub fn db_pool<'a>() -> &'a PgPool {
    POOL.get().expect("pool should be set at server start")
}

pub fn yt_api_key<'a>() -> &'a str {
    YOUTUBE_API_KEY
        .get()
        .expect("youtube api key should be set at server start")
}

pub fn brain_addr<'a>() -> &'a Addr<AudioBrain> {
    BRAIN_ADDR
        .get()
        .expect("brain address should be set at server start")
}

#[cfg(test)]
pub mod tests_utils;
