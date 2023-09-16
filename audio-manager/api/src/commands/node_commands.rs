use actix::Message;
use actix_web::{
    http::StatusCode,
    post,
    web::{self, Data},
    HttpResponse,
};
use serde::Deserialize;

use crate::{
    audio_item::AudioMetaData, audio_player::LoopBounds, utils::get_node_by_source_name, AppData,
    ErrorResponse,
};

/// Commands a client can send to an audio node
///
/// # Example commands
///
/// { "ADD_QUEUE_ITEM": { "metadata": { "name": "the pretender" }, "url": "https://www.youtube.com/watch?v=SBjQ9tuuTJQ"} }
/// { "ADD_QUEUE_ITEM": { "metadata": { "name": "the teacher" }, "url": "https://www.youtube.com/watch?v=6MF6trC529M"} }
/// "PLAY_NEXT"
///
#[derive(Debug, Clone, Deserialize, Message)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[rtype(result = "Result<(), ErrorResponse>")]
pub enum AudioNodeCommand {
    AddQueueItem(AddQueueItemParams),
    RemoveQueueItem(RemoveQueueItemParams),
    MoveQueueItem(MoveQueueItemParams),
    SetAudioProgress(SetAudioProgressParams),
    PauseQueue,
    UnPauseQueue,
    PlayNext,
    PlayPrevious,
    PlaySelected(PlaySelectedParams),
    LoopQueue(LoopQueueParams),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddQueueItemParams {
    pub metadata: AudioMetaData,
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveQueueItemParams {
    pub index: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaySelectedParams {
    pub index: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveQueueItemParams {
    pub old_pos: usize,
    pub new_pos: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetAudioProgressParams {
    pub progress: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
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

    match node_addr.try_send(cmd.into_inner()) {
        Ok(()) => HttpResponse::new(StatusCode::OK),
        Err(_) => HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR),
    }
}
