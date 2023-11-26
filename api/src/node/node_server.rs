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
    db_pool,
    downloader::{
        AudioDownloader, DownloadAudioRequest, DownloadIdentifier, NotifyDownloadFinished,
    },
    streams::node_streams::{
        AudioNodeInfoStreamMessage, AudioNodeInfoStreamType, AudioStateInfo, DownloadInfo,
    },
    utils::log_msg_received,
    ErrorResponse, IntoErrResp,
};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    thread,
    time::Duration,
};

use actix::{
    Actor, ActorFutureExt, Addr, AsyncContext, Context, Handler, Message, MessageResponse,
    Recipient, ResponseActFuture, WrapFuture,
};
use serde::Serialize;
use ts_rs::TS;

use super::node_session::{AudioNodeSession, NodeSessionWsResponse};

const DEVICE_RECOVERY_ATTEMPT_INTERVAL: Duration = Duration::from_secs(5);

pub type SourceName = String;

pub struct AudioNode {
    source_name: SourceName,
    current_processor_info: ProcessorInfo,
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
            player,
            downloader_addr,
            server_addr,
            active_downloads: HashSet::default(),
            failed_downloads: HashMap::default(),
            sessions: HashMap::default(),
            health: AudioNodeHealth::Good,
            current_processor_info: ProcessorInfo::new(1.0),
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

    fn multicast_result<MOk, MErr>(&self, msg: Result<MOk, MErr>)
    where
        MOk: Message + Send + Clone + 'static,
        MOk::Result: Send,
        AudioNodeSession: Handler<MOk>,
        MErr: Message + Send + Clone + 'static,
        MErr::Result: Send,
        AudioNodeSession: Handler<MErr>,
    {
        match msg {
            Ok(msg) => {
                for addr in self.sessions.values() {
                    addr.do_send(msg.clone());
                }
            }
            Err(msg) => {
                for addr in self.sessions.values() {
                    addr.do_send(msg.clone());
                }
            }
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
            queue: msg
                .wanted_info
                .contains(&AudioNodeInfoStreamType::Queue)
                .then_some(extract_queue_metadata(self.player.queue())),
            health: msg
                .wanted_info
                .contains(&AudioNodeInfoStreamType::Health)
                .then_some(self.health.clone()),
            downloads: msg
                .wanted_info
                .contains(&AudioNodeInfoStreamType::Download)
                .then_some(DownloadInfo {
                    active: self.active_downloads.clone().into_iter().collect(),
                    failed: self.failed_downloads.clone().into_iter().collect(),
                }),
            audio_state_info: msg
                .wanted_info
                .contains(&AudioNodeInfoStreamType::AudioStateInfo)
                .then_some(AudioStateInfo {
                    playback_info: PlaybackInfo {
                        current_head_index: self.player.queue_head(),
                    },
                    processor_info: self.current_processor_info.clone(),
                }),
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
            Ok((identifier, metadata)) => {
                self.active_downloads.remove(&identifier);
                self.failed_downloads.remove(&identifier);

                let item = AudioPlayerQueueItem {
                    metadata,
                    locator: identifier.to_path_with_ext(),
                };

                if let Err(err) = self.player.push_to_queue(item) {
                    log::error!("failed to auto play first song, ERROR: {err}");
                    return;
                };

                let download_fin_msg = AudioNodeInfoStreamMessage::Download(DownloadInfo {
                    active: self.active_downloads.clone().into_iter().collect(),
                    failed: self.failed_downloads.clone().into_iter().collect(),
                });
                self.multicast(download_fin_msg);

                let updated_queue_msg =
                    AudioNodeInfoStreamMessage::Queue(extract_queue_metadata(self.player.queue()));
                self.multicast(updated_queue_msg);
            }
            Err((identifier, err_resp)) => {
                self.active_downloads.remove(&identifier);
                self.failed_downloads.insert(identifier, err_resp);

                let msg = AudioNodeInfoStreamMessage::Download(DownloadInfo {
                    active: self.active_downloads.clone().into_iter().collect(),
                    failed: self.failed_downloads.clone().into_iter().collect(),
                });

                self.multicast(msg);
            }
        }
    }
}

#[derive(Debug, Clone, Message)]
#[rtype(result = "()")]
enum AsyncAudioNodeCommand {
    AddQueueItem(AddQueueItemParams),
}

impl Handler<AsyncAudioNodeCommand> for AudioNode {
    type Result = ResponseActFuture<Self, ()>;

    fn handle(&mut self, msg: AsyncAudioNodeCommand, _ctx: &mut Self::Context) -> Self::Result {
        match msg {
            AsyncAudioNodeCommand::AddQueueItem(params) => {
                let AddQueueItemParams { identifier } = params;
                let uid = identifier.uid();

                Box::pin(async move {
                    sqlx::query_as!(
                AudioMetaData,
                "SELECT name, author, duration, cover_art_url FROM audio_metadata where identifier = $1",
                uid
                    )
                    .fetch_optional(db_pool())
                    .await
                    .into_err_resp("")

                }.into_actor(self).map(move |res, act, ctx| {
                    match res {
                        Ok(metadata) => {
                            let msg = handle_add_queue_item(
                                metadata,
                                identifier.clone(),
                                act,
                                ctx.address().recipient(),
                            );

                            act.multicast_result(msg);
                        },
                        Err(err_resp) => {act.multicast(err_resp);}
                    }

                }))
            } // _ => Box::pin(async { Ok(()) }.into_actor(self)),
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

                ctx.notify(AsyncAudioNodeCommand::AddQueueItem(params.clone()));
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

                self.player.play_next().into_err_resp("")?;
                Ok(())
            }
            AudioNodeCommand::PlayPrevious => {
                log::info!("'PlayPrevious' handler received a message, MESSAGE: {msg:?}");

                self.player.play_prev().into_err_resp("")?;
                Ok(())
            }
            AudioNodeCommand::PlaySelected(params) => {
                log::info!("'PlaySelected' handler received a message, MESSAGE: {msg:?}");

                self.player
                    .play_selected(params.index, false)
                    .into_err_resp("")?;
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
                self.current_processor_info = processor_info.clone();

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
                let device_health_restored = if let Err(err) = self
                    .player
                    .try_recover_device(self.current_processor_info.audio_progress)
                {
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
    metadata: Option<AudioMetaData>,
    identifier: DownloadIdentifier,
    node: &mut AudioNode,
    node_addr: Recipient<NotifyDownloadFinished>,
) -> Result<AudioNodeInfoStreamMessage, ErrorResponse> {
    if let Some(metadata) = metadata {
        if let Err(err) = node.player.push_to_queue(AudioPlayerQueueItem {
            metadata,
            locator: identifier.to_path_with_ext(),
        }) {
            log::error!("failed to auto play first song");
            return Err(ErrorResponse {
                error: format!("failed to auto play first song, ERROR: {err}"),
            });
        }
    } else {
        let download_request = node.downloader_addr.try_send(DownloadAudioRequest {
            addr: node_addr,
            identifier: identifier.clone(),
        });

        if download_request.is_ok() {
            node.active_downloads.insert(identifier);

            return Ok(AudioNodeInfoStreamMessage::Download(DownloadInfo {
                active: node.active_downloads.clone().into_iter().collect(),
                failed: node.failed_downloads.clone().into_iter().collect(),
            }));
        }
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
