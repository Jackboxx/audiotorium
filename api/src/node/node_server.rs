use crate::{
    audio::{
        audio_item::{AudioDataLocator, AudioPlayerQueueItem},
        audio_player::{
            AudioPlayer, PlaybackInfo, PlaybackState, ProcessorInfo, SerializableQueue,
        },
    },
    brain::brain_server::{AudioBrain, AudioNodeToBrainMessage},
    commands::node_commands::{
        AddQueueItemParams, AudioNodeCommand, MoveQueueItemParams, RemoveQueueItemParams,
    },
    downloader::{
        AudioDownloader, DownloadAudioRequest, DownloadIdentifier, NotifyDownloadFinished,
    },
    streams::node_streams::{AudioNodeInfoStreamMessage, AudioNodeInfoStreamType, AudioStateInfo},
    utils::log_msg_received,
    ErrorResponse,
};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    thread,
    time::Duration,
};

use actix::{Actor, Addr, AsyncContext, Context, Handler, Message, MessageResponse, Recipient};
use serde::Serialize;
use ts_rs::TS;

use super::node_session::{AudioNodeSession, NodeSessionWsResponse};

const DEVICE_RECOVERY_ATTEMPT_INTERVAL: Duration = Duration::from_secs(5);

pub type SourceName = String;

pub struct AudioNode {
    source_name: SourceName,
    current_audio_progress: f64,
    player: AudioPlayer<PathBuf>,
    downloader_addr: Addr<AudioDownloader>,
    active_downloads: HashSet<DownloadIdentifier>,
    failed_downloads: HashMap<DownloadIdentifier, ErrorResponse>,
    server_addr: Addr<AudioBrain>,
    sessions: HashMap<usize, Addr<AudioNodeSession>>,
    health: AudioNodeHealth,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../app/src/api-types/")]
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

#[derive(Debug, Clone, Serialize, PartialEq, Eq, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "../app/src/api-types/")]
pub enum AudioNodeHealth {
    Good,
    Mild(AudioNodeHealthMild),
    Poor(AudioNodeHealthPoor),
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "../app/src/api-types/")]
pub enum AudioNodeHealthMild {
    Buffering,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "../app/src/api-types/")]
pub enum AudioNodeHealthPoor {
    DeviceNotAvailable,
    AudioStreamReadFailed,
    AudioBackendError(String),
}

#[derive(Debug, Clone, Message)]
#[rtype(result = "NodeConnectResponse")]
pub struct NodeConnectMessage {
    pub addr: Addr<AudioNodeSession>,
    pub wanted_info: Vec<AudioNodeInfoStreamType>,
}

#[derive(Debug, Clone, Message)]
#[rtype(result = "()")]
pub struct NodeDisconnectMessage {
    pub id: usize,
}

#[derive(Debug, Clone, MessageResponse)]
pub struct NodeConnectResponse {
    pub id: usize,
    pub connection_response: NodeSessionWsResponse,
}

#[derive(Debug, Clone, Message)]
#[rtype(result = "()")]
pub struct TryRecoverDevice;

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
            active_downloads: HashSet::default(),
            failed_downloads: HashMap::default(),
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
        log_msg_received(&self, &msg);

        let id = self.sessions.keys().max().unwrap_or(&0) + 1;
        self.sessions.insert(id, msg.addr);

        let connection_response = NodeSessionWsResponse::SessionConnectedResponse {
            queue: if msg.wanted_info.contains(&AudioNodeInfoStreamType::Queue) {
                Some(extract_queue_metadata(self.player.queue()))
            } else {
                None
            },
            health: if msg.wanted_info.contains(&AudioNodeInfoStreamType::Health) {
                Some(self.health.clone())
            } else {
                None
            },
        };

        NodeConnectResponse {
            id,
            connection_response,
        }
    }
}

impl Handler<NodeDisconnectMessage> for AudioNode {
    type Result = ();

    fn handle(&mut self, msg: NodeDisconnectMessage, _ctx: &mut Self::Context) -> Self::Result {
        log_msg_received(&self, &msg);

        self.sessions.remove(&msg.id);
    }
}

impl Handler<NotifyDownloadFinished> for AudioNode {
    type Result = ();

    fn handle(&mut self, msg: NotifyDownloadFinished, _ctx: &mut Self::Context) -> Self::Result {
        match msg.result {
            Ok(identifier) => {
                self.active_downloads.remove(&identifier);
                self.failed_downloads.remove(&identifier);

                // TODO: add db to handle metadata storing
                let item = AudioPlayerQueueItem {
                    metadata: crate::audio::audio_item::AudioMetaData {
                        name: String::new(),
                        author: None,
                        duration: None,
                        thumbnail_url: None,
                    },
                    locator: identifier.to_path_with_ext(),
                };

                if let Err(err) = self.player.push_to_queue(item) {
                    log::error!("failed to auto play first song, ERROR: {err}");
                    return;
                };

                let download_fin_msg = AudioNodeInfoStreamMessage::Download {
                    active: self.active_downloads.clone().into_iter().collect(),
                    failed: self.failed_downloads.clone().into_iter().collect(),
                };
                self.multicast(download_fin_msg);

                let updated_queue_msg =
                    AudioNodeInfoStreamMessage::Queue(extract_queue_metadata(self.player.queue()));
                self.multicast(updated_queue_msg);
            }
            Err((identifier, err_resp)) => {
                self.active_downloads.remove(&identifier);
                self.failed_downloads.insert(identifier, err_resp);

                let msg = AudioNodeInfoStreamMessage::Download {
                    active: self.active_downloads.clone().into_iter().collect(),
                    failed: self.failed_downloads.clone().into_iter().collect(),
                };

                self.multicast(msg);
            }
        }
    }
}

impl Handler<AudioNodeCommand> for AudioNode {
    type Result = Result<(), ErrorResponse>;

    fn handle(&mut self, msg: AudioNodeCommand, ctx: &mut Self::Context) -> Self::Result {
        log_msg_received(&self, &msg);

        match &msg {
            AudioNodeCommand::AddQueueItem(params) => {
                log::info!("'AddQueueItem' handler received a message, MESSAGE: {msg:?}");

                let msg = handle_add_queue_item(self, ctx.address().recipient(), params.clone())?;
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
            AudioNodeCommand::SetAudioVolume(params) => {
                log::info!("'SetAudioVolume' handler received a message, MESSAGE: {msg:?}");

                self.player.set_volume(params.volume);
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
                    .play_selected(params.index, false)
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
        ctx: &mut Self::Context,
    ) -> Self::Result {
        match msg {
            AudioProcessorToNodeMessage::AudioStateInfo(_) => {}
            _ => {
                log_msg_received(&self, &msg);
            }
        }

        match msg {
            AudioProcessorToNodeMessage::Health(health) => {
                self.health = health.clone();

                self.server_addr
                    .do_send(AudioNodeToBrainMessage::NodeHealthUpdate((
                        self.source_name.to_owned(),
                        health.clone(),
                    )));

                self.multicast(AudioNodeInfoStreamMessage::Health(health));

                match self.health {
                    AudioNodeHealth::Good => {}
                    _ => {
                        if let Err(err) = ctx.address().try_send(TryRecoverDevice) {
                            log::error!(
                                "failed to send initial 'try device revocer' message\nERROR: {err}"
                            );
                        }
                    }
                };
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

impl Handler<TryRecoverDevice> for AudioNode {
    type Result = ();

    #[allow(clippy::collapsible_else_if)]
    fn handle(&mut self, _msg: TryRecoverDevice, ctx: &mut Self::Context) -> Self::Result {
        match self.health {
            AudioNodeHealth::Good => {}
            _ => {
                let device_health_restored =
                    if let Err(err) = self.player.try_recover_device(self.current_audio_progress) {
                        log::error!(
                            "failed to recover device for node with source name {}\nERROR: {err}",
                            self.source_name
                        );
                        false
                    } else {
                        true
                    };

                if !device_health_restored {
                    thread::sleep(DEVICE_RECOVERY_ATTEMPT_INTERVAL);

                    if let Err(err) = ctx.address().try_send(TryRecoverDevice) {
                        log::error!("failed to resend 'try device revocer' message\nERROR: {err}");
                    };
                } else {
                    if let Err(err) = ctx
                        .address()
                        .try_send(AudioProcessorToNodeMessage::Health(AudioNodeHealth::Good))
                    {
                        log::error!(
                            "failed to inform node with source name {} of recovery\nERROR: {err}",
                            self.source_name
                        )
                    };
                }
            }
        };
    }
}

fn handle_add_queue_item(
    node: &mut AudioNode,
    node_addr: Recipient<NotifyDownloadFinished>,
    params: AddQueueItemParams,
) -> Result<AudioNodeInfoStreamMessage, ErrorResponse> {
    let AddQueueItemParams { identifier } = params.clone();

    let path = identifier.to_path_with_ext();

    if !path.try_exists().unwrap_or(false) {
        if let Ok(()) = node.downloader_addr.try_send(DownloadAudioRequest {
            addr: node_addr,
            identifier: identifier.clone(),
        }) {
            node.active_downloads.insert(identifier);

            return Ok(AudioNodeInfoStreamMessage::Download {
                active: node.active_downloads.clone().into_iter().collect(),
                failed: node.failed_downloads.clone().into_iter().collect(),
            });
        }
    } else if let Err(err) = node.player.push_to_queue(AudioPlayerQueueItem {
        metadata: crate::audio::audio_item::AudioMetaData {
            name: String::new(),
            author: None,
            duration: None,
            thumbnail_url: None,
        },
        locator: path,
    }) {
        log::error!("failed to auto play first song, MESSAGE: {params:?}, ERROR: {err}");
        return Err(ErrorResponse {
            error: format!("failed to auto play first song, ERROR: {err}"),
        });
    }

    Ok(AudioNodeInfoStreamMessage::Queue(extract_queue_metadata(
        node.player.queue(),
    )))
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
