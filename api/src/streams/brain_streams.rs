use std::sync::Arc;

use actix::Message;
use actix_web::{get, http::StatusCode, web, HttpRequest, HttpResponse};
use actix_web_actors::ws;
use clap::ValueEnum;
use serde::{Deserialize, Serialize};

use crate::{
    brain::brain_session::AudioBrainSession, brain_addr, node::node_server::AudioNodeInfo,
    streams::deserialize_stringified_list,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AudioBrainInfoStreamType {
    NodeInfo,
}

#[derive(Debug, Clone, Serialize, Message)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[rtype(result = "()")]
pub enum AudioBrainInfoStreamMessage {
    NodeInfo(Arc<[AudioNodeInfo]>),
}

#[derive(Debug, Clone, Deserialize)]
struct StreamWantedInfoParams {
    #[serde(deserialize_with = "deserialize_stringified_list")]
    wanted_info: Arc<[AudioBrainInfoStreamType]>,
}

pub fn get_type_of_stream_data(msg: &AudioBrainInfoStreamMessage) -> AudioBrainInfoStreamType {
    match msg {
        AudioBrainInfoStreamMessage::NodeInfo(_) => AudioBrainInfoStreamType::NodeInfo,
    }
}

#[get("/streams/brain")]
async fn get_brain_stream(
    query: web::Query<StreamWantedInfoParams>,
    req: HttpRequest,
    stream: web::Payload,
) -> HttpResponse {
    match ws::start(
        AudioBrainSession::new(brain_addr().clone(), query.into_inner().wanted_info),
        &req,
        stream,
    ) {
        Ok(res) => res,
        Err(_) => HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR),
    }
}
