use actix::Message;
use actix_web::{
    get,
    http::StatusCode,
    web::{self, Data},
    HttpRequest, HttpResponse,
};
use actix_web_actors::ws;
use serde::{Deserialize, Serialize};

use crate::{
    audio::audio_player::{PlaybackInfo, ProcessorInfo, SerializableQueue},
    node::{node_server::AudioNodeHealth, node_session::AudioNodeSession},
    streams::StreamWantedInfoParams,
    utils::get_node_by_source_name,
    AppData,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AudioNodeInfoStreamType {
    Queue,
    Health,
    Download,
    AudioStateInfo,
}

#[derive(Debug, Clone, Serialize, Message)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[rtype(result = "()")]
pub enum AudioNodeInfoStreamMessage {
    Queue(SerializableQueue),
    Health(AudioNodeHealth),
    Download(DownloadInfo),
    AudioStateInfo(AudioStateInfo),
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadInfo {
    pub in_progress: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AudioStateInfo {
    pub playback_info: PlaybackInfo,
    pub processor_info: ProcessorInfo,
}

pub fn get_type_of_stream_data(msg: &AudioNodeInfoStreamMessage) -> AudioNodeInfoStreamType {
    match msg {
        AudioNodeInfoStreamMessage::Queue(_) => AudioNodeInfoStreamType::Queue,
        AudioNodeInfoStreamMessage::Health(_) => AudioNodeInfoStreamType::Health,
        AudioNodeInfoStreamMessage::Download(_) => AudioNodeInfoStreamType::Download,
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
    let node_addr = match get_node_by_source_name(source_name.into_inner(), &data.brain_addr).await
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
