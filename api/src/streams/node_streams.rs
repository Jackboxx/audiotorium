use actix::Message;
use actix_web::{
    get,
    http::StatusCode,
    web::{self, Data},
    HttpRequest, HttpResponse,
};
use actix_web_actors::ws;
use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{
    audio_playback::{
        audio_item::AudioMetadata,
        audio_player::{PlaybackInfo, ProcessorInfo},
    },
    downloader::info::DownloadInfo,
    node::{health::AudioNodeHealth, node_session::AudioNodeSession},
    streams::deserialize_stringified_list,
    utils::get_node_by_source_name,
    AppData, ErrorResponse,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AudioNodeInfoStreamType {
    Queue,
    Health,
    Download,
    AudioStateInfo,
}

#[derive(Debug, Clone, Serialize, TS, Message)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[rtype(result = "()")]
#[ts(export, export_to = "../app/src/api-types/")]
pub enum AudioNodeInfoStreamMessage {
    // can't use SerializableQueue due to issue discussed
    // here: https://github.com/Aleph-Alpha/ts-rs/issues/70
    Queue(Vec<AudioMetadata>),
    Health(AudioNodeHealth),
    Download(RunningDownloadInfo),
    AudioStateInfo(AudioStateInfo),
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../app/src/api-types/")]
pub struct RunningDownloadInfo {
    pub active: Vec<DownloadInfo>,
    pub failed: Vec<(DownloadInfo, ErrorResponse)>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../app/src/api-types/")]
pub struct AudioStateInfo {
    pub playback_info: PlaybackInfo,
    pub processor_info: ProcessorInfo,
}

#[derive(Debug, Clone, Deserialize)]
struct StreamWantedInfoParams {
    #[serde(deserialize_with = "deserialize_stringified_list")]
    wanted_info: Vec<AudioNodeInfoStreamType>,
}

pub fn get_type_of_stream_data(msg: &AudioNodeInfoStreamMessage) -> AudioNodeInfoStreamType {
    match msg {
        AudioNodeInfoStreamMessage::Queue(_) => AudioNodeInfoStreamType::Queue,
        AudioNodeInfoStreamMessage::Health(_) => AudioNodeInfoStreamType::Health,
        AudioNodeInfoStreamMessage::Download { .. } => AudioNodeInfoStreamType::Download,
        AudioNodeInfoStreamMessage::AudioStateInfo(_) => AudioNodeInfoStreamType::AudioStateInfo,
    }
}

#[get("/streams/node/{source_name}")]
async fn get_node_stream(
    data: Data<AppData>,
    source_name: web::Path<String>,
    query: web::Query<StreamWantedInfoParams>,
    req: HttpRequest,
    stream: web::Payload,
) -> HttpResponse {
    let node_addr = match get_node_by_source_name(source_name.into_inner(), data.brain_addr()).await
    {
        Some(addr) => addr,
        None => {
            return HttpResponse::new(StatusCode::NOT_FOUND);
        }
    };

    match ws::start(
        AudioNodeSession::new(node_addr, query.into_inner().wanted_info),
        &req,
        stream,
    ) {
        Ok(res) => res,
        Err(_) => HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR),
    }
}
