use std::{fmt::Display, sync::OnceLock};

use actix::{Addr, Message};
use brain::brain_server::AudioBrain;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use ts_rs::TS;

pub mod commands;
pub mod streams;

pub mod audio_playback;
pub mod brain;
pub mod downloader;
pub mod message_send_handler;
pub mod node;
pub mod utils;

pub static POOL: OnceLock<PgPool> = OnceLock::new(); // set on server start

pub fn db_pool<'a>() -> &'a PgPool {
    POOL.get().expect("pool should be set at server start")
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

#[derive(Debug, Clone, Serialize, Deserialize, TS, Message)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../app/src/api-types/")]
#[rtype(result = "()")]
pub struct ErrorResponse {
    error: String,
}

trait IntoErrResp<R> {
    fn into_err_resp(self, details: &str) -> R;
}

impl<E: Display> IntoErrResp<ErrorResponse> for E {
    fn into_err_resp(self, details: &str) -> ErrorResponse {
        ErrorResponse {
            error: format!("{details} {self}"),
        }
    }
}

impl<T, E> IntoErrResp<Result<T, ErrorResponse>> for Result<T, E>
where
    E: IntoErrResp<ErrorResponse>,
{
    fn into_err_resp(self, details: &str) -> Result<T, ErrorResponse> {
        self.map_err(|err| err.into_err_resp(details))
    }
}
