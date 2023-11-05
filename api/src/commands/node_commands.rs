use actix::Message;
use actix_web::{
    http::StatusCode,
    post,
    web::{self, Data},
    HttpResponse,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{
    audio::audio_player::LoopBounds, downloader::DownloadIdentifier,
    utils::get_node_by_source_name, AppData, ErrorResponse,
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
#[rtype(result = "Result<(), ErrorResponse>")]
pub enum AudioNodeCommand {
    AddQueueItem(AddQueueItemParams),
    RemoveQueueItem(RemoveQueueItemParams),
    MoveQueueItem(MoveQueueItemParams),
    SetAudioVolume(SetAudioVolumeParams),
    SetAudioProgress(SetAudioProgressParams),
    PauseQueue,
    UnPauseQueue,
    PlayNext,
    PlayPrevious,
    PlaySelected(PlaySelectedParams),
    LoopQueue(LoopQueueParams),
}

#[derive(Debug, Clone, Serialize, TS, Deserialize)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../app/src/api-types/")]
pub struct AddQueueItemParams {
    pub identifier: DownloadIdentifier,
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

#[derive(Debug, Clone, Serialize, TS, Deserialize)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../app/src/api-types/")]
pub struct LoopQueueParams {
    pub bounds: Option<LoopBounds>,
}

#[post("/commands/node/{source_name}")]
pub async fn receive_node_cmd(
    data: Data<AppData>,
    source_name: web::Path<String>,
    cmd: web::Json<AudioNodeCommand>,
) -> HttpResponse {
    let node_addr = match get_node_by_source_name(source_name.into_inner(), &data.brain_addr).await
    {
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
