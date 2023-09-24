use crate::{
    audio::{
        audio_item::{AudioDataLocator, AudioMetaData, AudioPlayerQueueItem},
        audio_player::{
            AudioPlayer, PlaybackInfo, PlaybackState, ProcessorInfo, SerializableQueue,
        },
    },
    brain::brain_server::{AudioBrain, AudioNodeToBrainMessage},
    commands::node_commands::{
        AddQueueItemParams, AudioNodeCommand, MoveQueueItemParams, RemoveQueueItemParams,
    },
    downloader::{AudioDownloader, DownloadAudio, NotifyDownloadFinished},
    streams::node_streams::{AudioNodeInfoStreamMessage, AudioStateInfo, DownloadInfo},
    ErrorResponse, AUDIO_DIR,
};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use actix::{Actor, Addr, AsyncContext, Context, Handler, Message, MessageResponse, Recipient};
use serde::Serialize;

use super::node_session::AudioNodeSession;

pub struct AudioNode {
    source_name: String,
    current_audio_progress: f64,
    player: AudioPlayer<PathBuf>,
    downloader_addr: Addr<AudioDownloader>,
    server_addr: Addr<AudioBrain>,
    sessions: HashMap<usize, Addr<AudioNodeSession>>,
    health: AudioNodeHealth,
}

#[derive(Debug, Clone, Serialize)]
pub struct AudioNodeInfo {
    pub source_name: String,
    pub human_readable_name: String,
    pub health: AudioNodeHealth,
}

#[derive(Debug, Clone, Message, PartialEq)]
#[rtype(result = "()")]
pub enum AudioProcessorToNodeMessage {
    AudioStateInfo(ProcessorInfo),
    Health(AudioNodeHealth),
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub enum AudioNodeHealth {
    Good,
    Mild(AudioNodeHealthMild),
    Poor(AudioNodeHealthPoor),
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub enum AudioNodeHealthMild {
    Buffering,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum AudioNodeHealthPoor {
    DeviceNotAvailable,
    AudioStreamReadFailed,
    AudioBackendError(String),
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
    pub queue: Vec<AudioMetaData>,
}

#[allow(clippy::enum_variant_names, dead_code)]
#[derive(Debug, Clone, Serialize, Message)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[rtype(result = "()")]
pub enum NodeInternalMessage {
    SendClientQueueQueueInfo(SendClientQueueInfoNodeResponse),
    StartedDownloadingAudio,
    FinishedDownloadingAudio(FinishedDownloadingAudioNodeResponse),
}

#[derive(Debug, Clone)]
pub struct SendClientQueueInfoNodeParams {
    pub processor_info: ProcessorInfo,
}

#[derive(Debug, Clone)]
pub struct UpdateNodeHealthParams {
    pub health: AudioNodeHealth,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SendClientQueueInfoNodeResponse {
    pub playback_info: PlaybackInfo,
    pub processor_info: ProcessorInfo,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FinishedDownloadingAudioNodeResponse {
    error: Option<String>,
    queue: Option<Vec<AudioMetaData>>,
}

impl AudioNode {
    pub fn new(
        source_name: String,
        player: AudioPlayer<PathBuf>,
        server_addr: Addr<AudioBrain>,
        downloader_addr: Addr<AudioDownloader>,
    ) -> Self {
        Self {
            source_name,
            current_audio_progress: 0.0,
            player,
            downloader_addr,
            server_addr,
            sessions: HashMap::default(),
            health: AudioNodeHealth::Good,
        }
    }

    fn multicast<M>(&self, msg: M)
    where
        M: Message + Send + Clone + 'static,
        M::Result: Send,
        AudioNodeSession: Handler<M>,
    {
        for addr in self.sessions.values() {
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
            queue: extract_queue_metadata(self.player.queue()),
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

        match msg.result {
            Ok(resp) => {
                if let Err(err) = self.player.push_to_queue(resp.item) {
                    log::error!("failed to auto play first song, ERROR: {err}");
                    return;
                };

                let download_fin_msg = AudioNodeInfoStreamMessage::Download(DownloadInfo {
                    in_progress: false,
                    error: None,
                });
                self.multicast(download_fin_msg);

                let updated_queue_msg =
                    AudioNodeInfoStreamMessage::Queue(extract_queue_metadata(self.player.queue()));
                self.multicast(updated_queue_msg);
            }
            Err(err_resp) => {
                let msg = AudioNodeInfoStreamMessage::Download(DownloadInfo {
                    in_progress: false,
                    error: Some(err_resp.error),
                });

                self.multicast(msg);
            }
        }
    }
}

impl Handler<AudioNodeCommand> for AudioNode {
    type Result = Result<(), ErrorResponse>;

    fn handle(&mut self, msg: AudioNodeCommand, ctx: &mut Self::Context) -> Self::Result {
        match &msg {
            AudioNodeCommand::AddQueueItem(params) => {
                log::info!("'AddQueueItem' handler received a message, MESSAGE: {msg:?}");

                let msg = AudioNodeInfoStreamMessage::Queue(handle_add_queue_item(
                    self,
                    ctx.address().recipient(),
                    params.clone(),
                )?);
                self.multicast(msg);

                Ok(())
            }
            AudioNodeCommand::RemoveQueueItem(params) => {
                log::info!("'RemoveQueueItem' handler received a message, MESSAGE: {msg:?}");

                let msg = AudioNodeInfoStreamMessage::Queue(handle_remove_queue_item(
                    self,
                    params.clone(),
                )?);
                self.multicast(msg);

                Ok(())
            }
            AudioNodeCommand::MoveQueueItem(params) => {
                log::info!("'MoveQueueItem' handler received a message, MESSAGE: {msg:?}");

                let msg =
                    AudioNodeInfoStreamMessage::Queue(handle_move_queue_item(self, params.clone()));

                self.multicast(msg);

                Ok(())
            }
            AudioNodeCommand::SetAudioProgress(params) => {
                log::info!("'SetAudioProgress' handler received a message, MESSAGE: {msg:?}");

                self.player.set_stream_progress(params.progress);
                Ok(())
            }
            AudioNodeCommand::PauseQueue => {
                log::info!("'PauseQueue' handler received a message, MESSAGE: {msg:?}");

                self.player.set_stream_playback_state(PlaybackState::Paused);
                Ok(())
            }
            AudioNodeCommand::UnPauseQueue => {
                log::info!("'UnPauseQueue' handler received a message, MESSAGE: {msg:?}");

                self.player
                    .set_stream_playback_state(PlaybackState::Playing);
                Ok(())
            }
            AudioNodeCommand::PlayNext => {
                log::info!("'PlayNext' handler received a message, MESSAGE: {msg:?}");

                self.player.play_next().map_err(|err| ErrorResponse {
                    error: err.to_string(),
                })?;
                Ok(())
            }
            AudioNodeCommand::PlayPrevious => {
                log::info!("'PlayPrevious' handler received a message, MESSAGE: {msg:?}");

                self.player.play_prev().map_err(|err| ErrorResponse {
                    error: err.to_string(),
                })?;
                Ok(())
            }
            AudioNodeCommand::PlaySelected(params) => {
                log::info!("'PlaySelected' handler received a message, MESSAGE: {msg:?}");

                self.player
                    .play_selected(params.index)
                    .map_err(|err| ErrorResponse {
                        error: err.to_string(),
                    })?;
                Ok(())
            }
            AudioNodeCommand::LoopQueue(params) => {
                log::info!("'LoopQueue' handler received a message, MESSAGE: {msg:?}");

                self.player.set_loop(params.bounds.clone());
                Ok(())
            }
        }
    }
}

impl Handler<AudioProcessorToNodeMessage> for AudioNode {
    type Result = ();

    fn handle(
        &mut self,
        msg: AudioProcessorToNodeMessage,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        match msg {
            AudioProcessorToNodeMessage::Health(health) => {
                let health_update = match (&self.health, &health) {
                    (AudioNodeHealth::Good, AudioNodeHealth::Poor(_)) => {
                        let device_health_restored =
                            self.player.try_recover_device(self.current_audio_progress);
                        if !device_health_restored {
                            Some(health)
                        } else {
                            log::info!("recovered device health for {}", self.source_name);
                            None
                        }
                    }
                    (
                        AudioNodeHealth::Mild(_) | AudioNodeHealth::Poor(_),
                        AudioNodeHealth::Poor(_),
                    ) => {
                        let device_health_restored =
                            self.player.try_recover_device(self.current_audio_progress);
                        if device_health_restored {
                            Some(AudioNodeHealth::Good)
                        } else {
                            Some(health)
                        }
                    }
                    _ => Some(health),
                };

                if let Some(health) = health_update {
                    self.health = health.clone();

                    self.server_addr
                        .do_send(AudioNodeToBrainMessage::NodeHealthUpdate((
                            self.source_name.to_owned(),
                            health.clone(),
                        )));

                    self.multicast(AudioNodeInfoStreamMessage::Health(health));
                }
            }
            AudioProcessorToNodeMessage::AudioStateInfo(processor_info) => {
                self.current_audio_progress = processor_info.audio_progress;

                let msg = AudioNodeInfoStreamMessage::AudioStateInfo(AudioStateInfo {
                    playback_info: PlaybackInfo {
                        current_head_index: self.player.queue_head(),
                    },
                    processor_info,
                });

                self.multicast(msg);
            }
        }
    }
}

fn handle_add_queue_item(
    node: &mut AudioNode,
    node_addr: Recipient<NotifyDownloadFinished>,
    params: AddQueueItemParams,
) -> Result<SerializableQueue, ErrorResponse> {
    let AddQueueItemParams { metadata, url } = params.clone();

    let path = Path::new(AUDIO_DIR).join(&metadata.name);
    let path_with_ext = path.clone().with_extension("mp3");

    if !path_with_ext.try_exists().unwrap_or(false) {
        let msg = AudioNodeInfoStreamMessage::Download(DownloadInfo {
            in_progress: true,
            error: None,
        });
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
    } else if let Err(err) = node.player.push_to_queue(AudioPlayerQueueItem {
        metadata,
        locator: path_with_ext,
    }) {
        log::error!("failed to auto play first song, MESSAGE: {params:?}, ERROR: {err}");
        return Err(ErrorResponse {
            error: format!("failed to auto play first song, ERROR: {err}"),
        });
    }

    Ok(extract_queue_metadata(node.player.queue()))
}

fn handle_remove_queue_item(
    node: &mut AudioNode,
    params: RemoveQueueItemParams,
) -> Result<SerializableQueue, ErrorResponse> {
    let RemoveQueueItemParams { index } = params.clone();

    if let Err(err) = node.player.remove_from_queue(index) {
        log::error!("failed to play correct audio after removing element from queue, MESSAGE: {params:?}, ERROR: {err}");
        return Err(ErrorResponse {
            error: format!("failed to play correct audio after removing element, ERROR: {err}"),
        });
    }

    Ok(extract_queue_metadata(node.player.queue()))
}

fn handle_move_queue_item(node: &mut AudioNode, params: MoveQueueItemParams) -> SerializableQueue {
    let MoveQueueItemParams { old_pos, new_pos } = params;
    node.player.move_queue_item(old_pos, new_pos);

    extract_queue_metadata(node.player.queue())
}

fn extract_queue_metadata<ADL: AudioDataLocator>(
    queue: &[AudioPlayerQueueItem<ADL>],
) -> SerializableQueue {
    queue.iter().map(|item| item.metadata.clone()).collect()
}
