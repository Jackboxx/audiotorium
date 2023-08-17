use crate::{
    audio::{LoopBounds, PlaybackState},
    downloader::{AudioDownloader, DownloadAudio, NotifyDownloadFinished},
    ErrorResponse, AUDIO_DIR,
};
use std::{
    collections::HashMap,
    ffi::OsStr,
    path::{Path, PathBuf},
};

use actix::{Actor, Addr, AsyncContext, Context, Handler, Message, MessageResponse, Recipient};
use serde::Serialize;

use crate::{
    audio::{AudioPlayer, PlaybackInfo, ProcessorInfo},
    node_session::AudioNodeSession,
};

pub struct AudioNode {
    player: AudioPlayer,
    downloader_addr: Addr<AudioDownloader>,
    sessions: HashMap<usize, Addr<AudioNodeSession>>,
}

#[derive(Debug, Clone, Message)]
#[rtype(result = "NodeConnectResponse")]
pub struct NodeConnectMessage {
    pub addr: Addr<AudioNodeSession>,
}

#[derive(Debug, Clone, Message)]
#[rtype(result = "()")]
pub struct NodeDisconnectMessage {
    pub id: usize,
}

#[derive(Debug, Clone, MessageResponse)]
pub struct NodeConnectResponse {
    pub id: usize,
    pub queue: Vec<String>,
}

#[derive(Debug, Clone, Message)]
#[rtype(result = "Result<(), ErrorResponse>")]
pub enum NodeInternalMessage {
    AddQueueItem(AddQueueItemNodeParams),
    RemoveQueueItem(RemoveQueueItemNodeParams),
    ReadQueueItems,
    MoveQueueItem(MoveQueueItemNodeParams),
    SetAudioProgress(SetAudioProgressNodeParams),
    PauseQueue,
    UnPauseQueue,
    PlayNext,
    PlayPrevious,
    PlaySelected(PlaySelectedNodeParams),
    LoopQueue(LoopQueueNodeParams),
    SendClientQueueInfo(SendClientQueueInfoNodeParams),
}

#[allow(clippy::enum_variant_names, dead_code)]
#[derive(Debug, Clone, Serialize, Message)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[rtype(result = "()")]
pub enum NodeInternalResponse {
    AddQueueItemResponse(AddQueueItemNodeResponse),
    RemoveQueueItemResponse(RemoveQueueItemNodeResponse),
    ReadQueueItemsResponse(ReadQueueNodeResponse),
    MoveQueueItemResponse(MoveQueueItemNodeResponse),
}

#[allow(clippy::enum_variant_names, dead_code)]
#[derive(Debug, Clone, Serialize, Message)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[rtype(result = "()")]
pub enum NodeInternalUpdateMessage {
    SendClientQueueQueueInfo(SendClientQueueInfoNodeResponse),
    StartedDownloadingAudio,
    FinishedDownloadingAudio(FinishedDownloadingAudioNodeResponse),
}

#[derive(Debug, Clone)]
pub struct SendClientQueueInfoNodeParams {
    pub processor_info: ProcessorInfo,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SendClientQueueInfoNodeResponse {
    pub playback_info: PlaybackInfo,
    pub processor_info: ProcessorInfo,
}

#[derive(Debug, Clone)]
pub struct AddQueueItemNodeParams {
    pub title: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddQueueItemNodeResponse {
    queue: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct RemoveQueueItemNodeParams {
    pub index: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveQueueItemNodeResponse {
    queue: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FinishedDownloadingAudioNodeResponse {
    error: Option<String>,
    queue: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadQueueNodeResponse {
    queue: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct MoveQueueItemNodeParams {
    pub old_pos: usize,
    pub new_pos: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveQueueItemNodeResponse {
    queue: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct SetAudioProgressNodeParams {
    pub progress: f64,
}

#[derive(Debug, Clone)]
pub struct PlaySelectedNodeParams {
    pub index: usize,
}

#[derive(Debug, Clone)]
pub struct LoopQueueNodeParams {
    pub bounds: Option<LoopBounds>,
}

impl AudioNode {
    pub fn new(player: AudioPlayer, downloader_addr: Addr<AudioDownloader>) -> Self {
        Self {
            player,
            downloader_addr,
            sessions: HashMap::default(),
        }
    }

    fn multicast<M>(&self, msg: M)
    where
        M: Message + Send + Clone + 'static,
        M::Result: Send,
        AudioNodeSession: Handler<M>,
    {
        for (_, addr) in &self.sessions {
            addr.do_send(msg.clone());
        }
    }
}

impl Actor for AudioNode {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        log::info!("stared new 'AudioNode', CONTEXT: {ctx:?}");

        self.player.set_addr(Some(ctx.address()))
    }
}

impl Handler<NodeConnectMessage> for AudioNode {
    type Result = NodeConnectResponse;

    fn handle(&mut self, msg: NodeConnectMessage, _ctx: &mut Self::Context) -> Self::Result {
        let id = self.sessions.keys().max().unwrap_or(&0) + 1;
        self.sessions.insert(id, msg.addr);
        NodeConnectResponse {
            id,
            queue: queue_as_string_vec(self.player.queue()),
        }
    }
}

impl Handler<NodeDisconnectMessage> for AudioNode {
    type Result = ();

    fn handle(&mut self, msg: NodeDisconnectMessage, _ctx: &mut Self::Context) -> Self::Result {
        self.sessions.remove(&msg.id);
    }
}

impl Handler<NotifyDownloadFinished> for AudioNode {
    type Result = ();

    fn handle(&mut self, msg: NotifyDownloadFinished, _ctx: &mut Self::Context) -> Self::Result {
        log::info!("'NotifyDownloadFinished' handler received a message, MESSAGE: {msg:?}");

        let msg = match msg.result {
            Ok(resp) => {
                if let Err(err) = self.player.push_to_queue(resp.path) {
                    log::error!("failed to auto play first song, ERROR: {err}");
                    return;
                };

                NodeInternalUpdateMessage::FinishedDownloadingAudio(
                    FinishedDownloadingAudioNodeResponse {
                        error: None,
                        queue: Some(queue_as_string_vec(self.player.queue())),
                    },
                )
            }
            Err(err_resp) => NodeInternalUpdateMessage::FinishedDownloadingAudio(
                FinishedDownloadingAudioNodeResponse {
                    error: Some(err_resp.error),
                    queue: None,
                },
            ),
        };

        self.multicast(msg);
    }
}

impl Handler<NodeInternalMessage> for AudioNode {
    type Result = Result<(), ErrorResponse>;

    fn handle(&mut self, msg: NodeInternalMessage, ctx: &mut Self::Context) -> Self::Result {
        match &msg {
            NodeInternalMessage::AddQueueItem(params) => {
                log::info!("'AddQueueItem' handler received a message, MESSAGE: {msg:?}");

                let msg = NodeInternalResponse::AddQueueItemResponse(handle_add_queue_item(
                    self,
                    ctx.address().recipient(),
                    params.clone(),
                )?);
                self.multicast(msg);

                Ok(())
            }
            NodeInternalMessage::RemoveQueueItem(params) => {
                log::info!("'RemoveQueueItem' handler received a message, MESSAGE: {msg:?}");

                let msg = NodeInternalResponse::RemoveQueueItemResponse(handle_remove_queue_item(
                    self,
                    params.clone(),
                )?);
                self.multicast(msg);

                Ok(())
            }
            NodeInternalMessage::ReadQueueItems => {
                log::info!("'ReadQueueItems' handler received a message, MESSAGE: {msg:?}");

                let msg = NodeInternalResponse::ReadQueueItemsResponse(handle_read_queue(&self));
                self.multicast(msg);

                Ok(())
            }
            NodeInternalMessage::MoveQueueItem(params) => {
                log::info!("'MoveQueueItem' handler received a message, MESSAGE: {msg:?}");

                let msg = NodeInternalResponse::MoveQueueItemResponse(handle_move_queue_item(
                    self,
                    params.clone(),
                ));

                self.multicast(msg);

                Ok(())
            }
            NodeInternalMessage::SetAudioProgress(params) => {
                log::info!("'SetAudioProgress' handler received a message, MESSAGE: {msg:?}");

                self.player.set_stream_progress(params.progress);
                Ok(())
            }
            NodeInternalMessage::PauseQueue => {
                log::info!("'PauseQueue' handler received a message, MESSAGE: {msg:?}");

                self.player.set_stream_playback_state(PlaybackState::Paused);
                Ok(())
            }
            NodeInternalMessage::UnPauseQueue => {
                log::info!("'UnPauseQueue' handler received a message, MESSAGE: {msg:?}");

                self.player
                    .set_stream_playback_state(PlaybackState::Playing);
                Ok(())
            }
            NodeInternalMessage::PlayNext => {
                log::info!("'PlayNext' handler received a message, MESSAGE: {msg:?}");

                self.player.play_next().map_err(|err| ErrorResponse {
                    error: err.to_string(),
                })?;
                Ok(())
            }
            NodeInternalMessage::PlayPrevious => {
                log::info!("'PlayPrevious' handler received a message, MESSAGE: {msg:?}");

                self.player.play_prev().map_err(|err| ErrorResponse {
                    error: err.to_string(),
                })?;
                Ok(())
            }
            NodeInternalMessage::PlaySelected(params) => {
                log::info!("'PlaySelected' handler received a message, MESSAGE: {msg:?}");

                self.player
                    .play_selected(params.index)
                    .map_err(|err| ErrorResponse {
                        error: err.to_string(),
                    })?;
                Ok(())
            }
            NodeInternalMessage::LoopQueue(params) => {
                log::info!("'LoopQueue' handler received a message, MESSAGE: {msg:?}");

                self.player.set_loop(params.bounds.clone());
                Ok(())
            }
            NodeInternalMessage::SendClientQueueInfo(params) => {
                log::info!("'SendClientQueueInfo' handler received a message, MESSAGE: {msg:?}");

                let msg = NodeInternalUpdateMessage::SendClientQueueQueueInfo(
                    SendClientQueueInfoNodeResponse {
                        playback_info: self.player.playback_info().clone(),
                        processor_info: params.processor_info.clone(),
                    },
                );

                self.multicast(msg);

                Ok(())
            }
        }
    }
}

fn handle_add_queue_item(
    node: &mut AudioNode,
    node_addr: Recipient<NotifyDownloadFinished>,
    params: AddQueueItemNodeParams,
) -> Result<AddQueueItemNodeResponse, ErrorResponse> {
    let AddQueueItemNodeParams { title, url } = params.clone();

    let path = Path::new(AUDIO_DIR).join(&title);
    let path_with_ext = path.clone().with_extension("mp3");

    if !path_with_ext.try_exists().unwrap_or(false) {
        let msg = NodeInternalUpdateMessage::StartedDownloadingAudio;
        node.multicast(msg);

        // TODO:
        // somehow this does not prevent the mailbox from being blocked even though this should
        // keep executing and not doing anything
        // this might be different now???

        node.downloader_addr.do_send(DownloadAudio {
            addr: node_addr,
            path,
            url,
        })
    } else {
        if let Err(err) = node.player.push_to_queue(path_with_ext) {
            log::error!("failed to auto play first song, MESSAGE: {params:?}, ERROR: {err}");
            return Err(ErrorResponse {
                error: format!("failed to auto play first song, ERROR: {err}"),
            });
        };
    }

    Ok(AddQueueItemNodeResponse {
        queue: queue_as_string_vec(node.player.queue()),
    })
}

fn handle_remove_queue_item(
    node: &mut AudioNode,
    params: RemoveQueueItemNodeParams,
) -> Result<RemoveQueueItemNodeResponse, ErrorResponse> {
    let RemoveQueueItemNodeParams { index } = params.clone();

    if let Err(err) = node.player.remove_from_queue(index) {
        log::error!("failed to play correct audio after removing element from queue, MESSAGE: {params:?}, ERROR: {err}");
        return Err(ErrorResponse {
            error: format!("failed to play correct audio after removing element, ERROR: {err}"),
        });
    }

    Ok(RemoveQueueItemNodeResponse {
        queue: queue_as_string_vec(node.player.queue()),
    })
}

fn handle_read_queue(node: &AudioNode) -> ReadQueueNodeResponse {
    ReadQueueNodeResponse {
        queue: queue_as_string_vec(node.player.queue()),
    }
}

fn handle_move_queue_item(
    node: &mut AudioNode,
    params: MoveQueueItemNodeParams,
) -> MoveQueueItemNodeResponse {
    let MoveQueueItemNodeParams { old_pos, new_pos } = params;
    node.player.move_queue_item(old_pos, new_pos);
    MoveQueueItemNodeResponse {
        queue: queue_as_string_vec(node.player.queue()),
    }
}

fn queue_as_string_vec(queue: &[PathBuf]) -> Vec<String> {
    queue
        .iter()
        .map(|path| {
            path.file_stem()
                .unwrap_or(OsStr::new(""))
                .to_str()
                .map(|str| str.to_owned())
                .unwrap_or(String::new())
        })
        .collect()
}
