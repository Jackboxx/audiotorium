use std::sync::Arc;

use actix::Message;
use actix_web::{http::StatusCode, post, web, HttpResponse};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{
    brain_addr, error::AppError, node::node_server::SourceName, utils::get_node_by_source_name,
};

/// Commands a client can send to an audio node
///
/// # Example commands
///
/// { "ADD_QUEUE_ITEM": { "metadata": { "name": "the pretender" }, "url": "https://www.youtube.com/watch?v=SBjQ9tuuTJQ"} }
/// { "ADD_QUEUE_ITEM": { "metadata": { "name": "the teacher" }, "url": "https://www.youtube.com/watch?v=6MF6trC529M"} }
/// "PLAY_NEXT"
///
#[derive(Debug, Clone, Serialize, TS, Deserialize, Message)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[ts(export, export_to = "../app/src/api-types/")]
#[rtype(result = "Result<(), AppError>")]
pub enum AudioNodeCommand {
    AddQueueItem(AddQueueItemParams),
    RemoveQueueItem(RemoveQueueItemParams),
    MoveQueueItem(MoveQueueItemParams),
    ShuffleQueue,
    SetAudioVolume(SetAudioVolumeParams),
    SetAudioProgress(SetAudioProgressParams),
    PauseQueue,
    UnPauseQueue,
    PlayNext,
    PlayPrevious,
    PlaySelected(PlaySelectedParams),
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "../app/src/api-types/")]
pub enum AudioIdentifier {
    Local { uid: Arc<str> },
    Youtube { url: Arc<str> },
}

#[derive(Debug, Clone, Serialize, TS, Deserialize)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../app/src/api-types/")]
pub struct AddQueueItemParams {
    pub identifier: AudioIdentifier,
}

#[derive(Debug, Clone, Serialize, TS, Deserialize)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../app/src/api-types/")]
pub struct RemoveQueueItemParams {
    pub index: usize,
}

#[derive(Debug, Clone, Serialize, TS, Deserialize)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../app/src/api-types/")]
pub struct PlaySelectedParams {
    pub index: usize,
}

#[derive(Debug, Clone, Serialize, TS, Deserialize)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../app/src/api-types/")]
pub struct MoveQueueItemParams {
    pub old_pos: usize,
    pub new_pos: usize,
}

#[derive(Debug, Clone, Serialize, TS, Deserialize)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../app/src/api-types/")]
pub struct SetAudioVolumeParams {
    pub volume: f32,
}

#[derive(Debug, Clone, Serialize, TS, Deserialize)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../app/src/api-types/")]
pub struct SetAudioProgressParams {
    pub progress: f64,
}

#[post("/commands/node/{source_name}")]
pub async fn receive_node_cmd(
    source_name: web::Path<SourceName>,
    cmd: web::Json<AudioNodeCommand>,
) -> HttpResponse {
    let node_addr = match get_node_by_source_name(source_name.into_inner(), brain_addr()).await {
        Some(addr) => addr,
        None => {
            return HttpResponse::new(StatusCode::NOT_FOUND);
        }
    };

    match node_addr.send(cmd.into_inner()).await {
        Ok(res) => match res {
            Ok(()) => HttpResponse::new(StatusCode::OK),
            Err(err) => HttpResponse::InternalServerError().body(
                serde_json::to_string(&err).unwrap_or("oops something went wrong".to_owned()),
            ),
        },
        Err(_) => HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR),
    }
}
