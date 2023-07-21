use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::Path;

use actix::{Actor, ActorContext, Addr, AsyncContext, Context, Handler, Message, Recipient};

use cpal::traits::{DeviceTrait, HostTrait};
use log::{error, info};
use serde::{Deserialize, Serialize};

use crate::{
    audio::{AudioSource, PlaybackInfo, ProcessorInfo},
    downloader::{
        self, AudioDownloader, DownloadAudio, DownloadAudioResponse, NotifyDownloadFinished,
    },
    session::{FilteredPassThroughtMessage, QueueSessionPassThroughMessages},
    ErrorResponse, AUDIO_DIR,
};

pub struct QueueServer {
    downloader_addr: Addr<AudioDownloader>,
    sources: HashMap<String, AudioSource>,
    sessions: HashMap<usize, Recipient<FilteredPassThroughtMessage>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum QueueServerMessage {
    AddQueueItem(AddQueueItemServerParams),
    RemoveQueueItem(RemoveQueueItemServerParams),
    ReadQueueItems(ReadQueueServerParams),
    MoveQueueItem(MoveQueueItemServerParams),
    AddSource(AddSourceServerParams),
    SetAudioProgress(SetAudioProgressServerParams),
    ReadSources(ReadSourcesServerParams),
    PauseQueue(PauseQueueServerParams),
    UnPauseQueue(UnPauseQueueServerParams),
    PlayNext(PlayNextServerParams),
    PlayPrevious(PlayPreviousServerParams),
    PlaySelected(PlaySelectedServerParams),
    LoopQueue(LoopQueueServerParams),
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[allow(clippy::enum_variant_names, dead_code)]
pub enum QueueServerMessageResponse {
    SessionConnectedResponse(ConnectServerResponse),
    AddQueueItemResponse(AddQueueItemServerResponse),
    RemoveQueueItemResponse(RemoveQueueItemServerResponse),
    FinishedDownloadingAudio(FinishedDownloadingAudioServerResponse),
    ReadQueueItemsResponse(ReadQueueServerResponse),
    MoveQueueItemResponse(MoveQueueItemServerResponse),
    AddSourceResponse(AddSourceServerResponse),
    ReadSourcesResponse(ReadSourcesServerResponse),
    SetAudioProgress(SetAudioProgressServerResponse),
    ReadSources(ReadSourcesServerResponse),
    PauseQueueResponse(PauseQueueServerResponse),
    UnPauseQueueResponse(UnPauseQueueServerResponse),
    PlayNextResponse(PlayNextServerResponse),
    PlayPreviousResponse(PlayPreviousServerResponse),
    PlaySelectedResponse(PlaySelectedServerResponse),
    LoopQueueResponse(LoopQueueServerResponse),
    SendClientQueueInfoResponse(SendClientQueueInfoServerResponse),
}

#[derive(Debug, Clone, Message)]
#[rtype(result = "Result<ConnectServerResponse, ()>")]
pub struct Connect {
    pub addr: Recipient<FilteredPassThroughtMessage>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConnectServerResponse {
    pub id: usize,
    pub sources: Vec<String>,
}

#[derive(Debug, Clone, Message)]
#[rtype(result = "()")]
pub struct Disconnect {
    pub id: usize,
}

#[derive(Debug, Clone, Deserialize, Message)]
#[rtype(result = "()")]
#[serde(rename_all = "camelCase")]
pub struct SendClientQueueInfoParams {
    pub source_name: String,
    pub processor_info: ProcessorInfo,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SendClientQueueInfoServerResponse {
    pub playback_info: PlaybackInfo,
    pub processor_info: ProcessorInfo,
}

#[derive(Debug, Clone, Deserialize, Message)]
#[rtype(result = "Result<AddQueueItemServerResponse, ErrorResponse>")]
#[serde(rename_all = "camelCase")]
pub struct AddQueueItemServerParams {
    pub source_name: String,
    pub title: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AddQueueItemServerResponse {
    queue: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Message)]
#[rtype(result = "Result<RemoveQueueItemServerResponse, ErrorResponse>")]
#[serde(rename_all = "camelCase")]
pub struct RemoveQueueItemServerParams {
    pub source_name: String,
    pub index: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct RemoveQueueItemServerResponse {
    queue: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FinishedDownloadingAudioServerResponse {
    error: Option<String>,
    queue: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize, Message)]
#[rtype(result = "Result<ReadQueueServerResponse, ErrorResponse>")]
#[serde(rename_all = "camelCase")]
pub struct ReadQueueServerParams {
    pub source_name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReadQueueServerResponse {
    queue: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Message)]
#[rtype(result = "Result<MoveQueueItemServerResponse, ErrorResponse>")]
#[serde(rename_all = "camelCase")]
pub struct MoveQueueItemServerParams {
    pub source_name: String,
    pub old_pos: usize,
    pub new_pos: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct MoveQueueItemServerResponse {
    queue: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Message)]
#[rtype(result = "Result<AddSourceServerResponse, ErrorResponse>")]
#[serde(rename_all = "camelCase")]
pub struct AddSourceServerParams {
    pub source_name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AddSourceServerResponse {
    sources: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Message)]
#[rtype(result = "Result<SetAudioProgressServerResponse, ErrorResponse>")]
#[serde(rename_all = "camelCase")]
pub struct SetAudioProgressServerParams {
    pub progress: f64,
    pub source_name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SetAudioProgressServerResponse;

#[derive(Debug, Clone, Deserialize, Message)]
#[rtype(result = "Result<ReadSourcesServerResponse, ErrorResponse>")]
#[serde(rename_all = "camelCase")]
pub struct ReadSourcesServerParams;

#[derive(Debug, Clone, Serialize)]
pub struct ReadSourcesServerResponse {
    pub sources: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Message)]
#[rtype(result = "Result<PauseQueueServerResponse, ErrorResponse>")]
#[serde(rename_all = "camelCase")]
pub struct PauseQueueServerParams {
    pub source_name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PauseQueueServerResponse;

#[derive(Debug, Clone, Deserialize, Message)]
#[rtype(result = "Result<UnPauseQueueServerResponse, ErrorResponse>")]
#[serde(rename_all = "camelCase")]
pub struct UnPauseQueueServerParams {
    pub source_name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnPauseQueueServerResponse;

#[derive(Debug, Clone, Deserialize, Message)]
#[rtype(result = "Result<PlayNextServerResponse, ErrorResponse>")]
#[serde(rename_all = "camelCase")]
pub struct PlayNextServerParams {
    pub source_name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayNextServerResponse;

#[derive(Debug, Clone, Deserialize, Message)]
#[rtype(result = "Result<PlayPreviousServerResponse, ErrorResponse>")]
#[serde(rename_all = "camelCase")]
pub struct PlayPreviousServerParams {
    pub source_name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayPreviousServerResponse;

#[derive(Debug, Clone, Deserialize, Message)]
#[rtype(result = "Result<PlaySelectedServerResponse, ErrorResponse>")]
#[serde(rename_all = "camelCase")]
pub struct PlaySelectedServerParams {
    pub source_name: String,
    pub index: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaySelectedServerResponse;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoopBounds {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, Deserialize, Message)]
#[rtype(result = "Result<LoopQueueServerResponse, ErrorResponse>")]
#[serde(rename_all = "camelCase")]
pub struct LoopQueueServerParams {
    pub source_name: String,
    pub bounds: Option<LoopBounds>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoopQueueServerResponse;

impl QueueServer {
    pub fn new(downloader_addr: Addr<AudioDownloader>) -> Self {
        Self {
            downloader_addr,
            sources: HashMap::default(),
            sessions: HashMap::default(),
        }
    }

    fn send_filtered_passthrought_msg(
        sessions: &[Recipient<FilteredPassThroughtMessage>],
        msg: String,
        source_name: &str,
    ) {
        for addr in sessions {
            addr.do_send(FilteredPassThroughtMessage {
                msg: msg.clone(),
                source_name: source_name.to_owned(),
            });
        }
    }
}

impl Actor for QueueServer {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        // check this if weird shit happens just trying stuff here
        ctx.set_mailbox_capacity(64);
        info!("stared new 'QueueServer', CONTEXT: {ctx:?}");

        match self.downloader_addr.try_send(downloader::Connect {
            addr: ctx.address().into(),
        }) {
            Ok(_) => {}
            Err(err) => {
                error!("'QueueServer' failed to connect to 'AudioDownloader', ERROR: {err}");
                ctx.stop();
            }
        };
    }
}

impl Handler<Connect> for QueueServer {
    type Result = Result<ConnectServerResponse, ()>;
    fn handle(&mut self, msg: Connect, _ctx: &mut Self::Context) -> Self::Result {
        let Connect { addr } = msg;
        let id = self.sessions.keys().max().unwrap_or(&0) + 1;

        self.sessions.insert(id, addr);

        Ok(ConnectServerResponse {
            id,
            sources: self.sources.keys().map(|key| key.to_owned()).collect(),
        })
    }
}

impl Handler<Disconnect> for QueueServer {
    type Result = ();
    fn handle(&mut self, msg: Disconnect, _ctx: &mut Self::Context) -> Self::Result {
        let Disconnect { id } = msg;
        self.sessions.remove(&id);
    }
}

impl Handler<SendClientQueueInfoParams> for QueueServer {
    type Result = ();

    fn handle(&mut self, msg: SendClientQueueInfoParams, _ctx: &mut Self::Context) -> Self::Result {
        let SendClientQueueInfoParams {
            source_name,
            processor_info,
        } = msg;
        if let Some(source) = self.sources.get(&source_name) {
            for session in &self.sessions {
                let addr = session.1;
                let msg = serde_json::to_string(
                    &QueueServerMessageResponse::SendClientQueueInfoResponse(
                        SendClientQueueInfoServerResponse {
                            playback_info: source.playback_info().clone(),
                            processor_info: processor_info.clone(),
                        },
                    ),
                )
                .unwrap_or(String::new());

                addr.do_send(FilteredPassThroughtMessage {
                    msg,
                    source_name: source_name.clone(),
                });
            }
        }
    }
}

impl Handler<AddQueueItemServerParams> for QueueServer {
    type Result = Result<AddQueueItemServerResponse, ErrorResponse>;

    fn handle(&mut self, msg: AddQueueItemServerParams, _ctx: &mut Self::Context) -> Self::Result {
        info!("'AddQueueItem' handler received a message, MESSAGE: {msg:?}");

        let AddQueueItemServerParams {
            source_name,
            title,
            url,
        } = msg.clone();
        let Some(source) = self.sources.get_mut(&source_name) else {
            error!("no audio source with the name {source_name} found, SOURCES: {:?}", self.sources.keys());
            return Err(ErrorResponse { error: format!("no audio source/source with the name {source_name} found") });
        };

        let path = Path::new(AUDIO_DIR).join(&title);
        let path_with_ext = path.clone().with_extension("mp3");

        let sessions = self
            .sessions
            .iter()
            .map(|(_k, v)| v.clone())
            .collect::<Vec<_>>();

        if !path_with_ext.try_exists().unwrap_or(false) {
            QueueServer::send_filtered_passthrought_msg(
                &sessions,
                serde_json::to_string(&QueueSessionPassThroughMessages::StartedDownloadingAudio)
                    .unwrap_or(String::new()),
                &source_name,
            );

            // somehow this does not prevent the mailbox from being blocked even though this should
            // keep executing and not doing anything
            self.downloader_addr.do_send(DownloadAudio {
                source_name: source_name.clone(),
                path,
                url,
            })
        } else {
            if let Err(err) = source.push_to_queue(path_with_ext, source_name) {
                error!("failed to auto play first song, MESSAGE: {msg:?}, ERROR: {err}");
                return Err(ErrorResponse {
                    error: format!("failed to auto play first song, ERROR: {err}"),
                });
            };
        }

        Ok(AddQueueItemServerResponse {
            queue: source
                .queue()
                .iter()
                .map(|path| {
                    path.file_stem()
                        .unwrap_or(OsStr::new(""))
                        .to_str()
                        .map(|str| str.to_owned())
                        .unwrap_or(String::new())
                })
                .collect(),
        })
    }
}

impl Handler<RemoveQueueItemServerParams> for QueueServer {
    type Result = Result<RemoveQueueItemServerResponse, ErrorResponse>;
    fn handle(
        &mut self,
        msg: RemoveQueueItemServerParams,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        info!("'RemoveQueueItem' handler received a message, MESSAGE: {msg:?}");

        let RemoveQueueItemServerParams { source_name, index } = msg.clone();
        let Some(source) = self.sources.get_mut(&source_name) else {
            error!("no audio source with the name {source_name} found, SOURCES: {:?}", self.sources.keys());
            return Err(ErrorResponse { error: format!("no audio source/source with the name {source_name} found") });
        };

        if let Err(err) = source.remove_from_queue(index, source_name) {
            error!("failed to play correct audio after removing element from queue, MESSAGE: {msg:?}, ERROR: {err}");
            return Err(ErrorResponse {
                error: format!("failed to play correct audio after removing element, ERROR: {err}"),
            });
        }

        Ok(RemoveQueueItemServerResponse {
            queue: source
                .queue()
                .iter()
                .map(|path| {
                    path.file_stem()
                        .unwrap_or(OsStr::new(""))
                        .to_str()
                        .map(|str| str.to_owned())
                        .unwrap_or(String::new())
                })
                .collect(),
        })
    }
}

impl Handler<NotifyDownloadFinished> for QueueServer {
    type Result = ();
    fn handle(&mut self, msg: NotifyDownloadFinished, _ctx: &mut Self::Context) -> Self::Result {
        info!("'NotifyDownloadFinished' handler received a message, MESSAGE: {msg:?}");

        let sessions = self
            .sessions
            .iter()
            .map(|(_k, v)| v.clone())
            .collect::<Vec<_>>();

        match msg.result {
            Ok(resp) => {
                let DownloadAudioResponse { source_name, path } = resp;

                let Some(source) = self.sources.get_mut(&source_name) else {
                    error!("no audio source with the name {source_name} found, SOURCES: {:?}", self.sources.keys());
                    return;
                };

                let path_with_ext = path.with_extension("mp3");
                if let Err(err) = source.push_to_queue(path_with_ext, source_name.clone()) {
                    error!("failed to auto play first song, ERROR: {err}");
                    return;
                };

                let resp = FinishedDownloadingAudioServerResponse {
                    error: None,
                    queue: Some(
                        source
                            .queue()
                            .iter()
                            .map(|path| {
                                path.file_stem()
                                    .unwrap_or(OsStr::new(""))
                                    .to_str()
                                    .map(|str| str.to_owned())
                                    .unwrap_or(String::new())
                            })
                            .collect(),
                    ),
                };

                QueueServer::send_filtered_passthrought_msg(
                    &sessions,
                    serde_json::to_string(
                        &QueueSessionPassThroughMessages::FinishedDownloadingAudio(resp),
                    )
                    .unwrap_or(String::new()),
                    &source_name,
                );
            }
            Err(err_resp) => {
                let resp = FinishedDownloadingAudioServerResponse {
                    error: Some(err_resp.1.error),
                    queue: None,
                };

                QueueServer::send_filtered_passthrought_msg(
                    &sessions,
                    serde_json::to_string(
                        &QueueSessionPassThroughMessages::FinishedDownloadingAudio(resp),
                    )
                    .unwrap_or(String::new()),
                    &err_resp.0,
                );
            }
        }
    }
}

impl Handler<ReadQueueServerParams> for QueueServer {
    type Result = Result<ReadQueueServerResponse, ErrorResponse>;
    fn handle(&mut self, msg: ReadQueueServerParams, _ctx: &mut Self::Context) -> Self::Result {
        info!("'ReadQueueItems' handler received a message, MESSAGE: {msg:?}");

        let ReadQueueServerParams { source_name } = msg;
        if let Some(source) = self.sources.get(&source_name) {
            Ok(ReadQueueServerResponse {
                queue: source
                    .queue()
                    .iter()
                    .map(|path| {
                        path.file_stem()
                            .unwrap_or(OsStr::new(""))
                            .to_str()
                            .map(|str| str.to_owned())
                            .unwrap_or(String::new())
                    })
                    .collect(),
            })
        } else {
            error!(
                "no audio source with the name {source_name} found, SOURCES: {:?}",
                self.sources.keys()
            );
            Err(ErrorResponse {
                error: format!("no audio source with the name {source_name} found"),
            })
        }
    }
}

impl Handler<MoveQueueItemServerParams> for QueueServer {
    type Result = Result<MoveQueueItemServerResponse, ErrorResponse>;
    fn handle(&mut self, msg: MoveQueueItemServerParams, _ctx: &mut Self::Context) -> Self::Result {
        info!("'MoveQueueItem' handler received a message, MESSAGE: {msg:?}");

        let MoveQueueItemServerParams {
            source_name,
            old_pos,
            new_pos,
        } = msg;

        if let Some(source) = self.sources.get_mut(&source_name) {
            if old_pos >= source.queue().len() {
                error!("'MoveQueueItem' params out of bounds");
                return Err(ErrorResponse {
                    error: "'oldPos' and 'newPos' out of bounds".to_owned(),
                });
            }

            source.move_queue_item(old_pos, new_pos);
            Ok(MoveQueueItemServerResponse {
                queue: source
                    .queue()
                    .iter()
                    .map(|path| {
                        path.file_stem()
                            .unwrap_or(OsStr::new(""))
                            .to_str()
                            .map(|str| str.to_owned())
                            .unwrap_or(String::new())
                    })
                    .collect(),
            })
        } else {
            error!(
                "no audio source with the name {source_name} found, SOURCES: {:?}",
                self.sources.keys()
            );
            Err(ErrorResponse {
                error: format!("no audio source with the name {source_name} found"),
            })
        }
    }
}

impl Handler<AddSourceServerParams> for QueueServer {
    type Result = Result<AddSourceServerResponse, ErrorResponse>;

    fn handle(&mut self, msg: AddSourceServerParams, ctx: &mut Self::Context) -> Self::Result {
        info!("'AddSource' handler received a message, MESSAGE: {msg:?}");

        let AddSourceServerParams { source_name } = msg;

        let host = cpal::default_host();
        let device = host
            .output_devices()
            .expect("no output device available")
            .find(|dev| dev.name().expect("device has no name") == source_name)
            .expect("no device found");

        let mut supported_configs_range = device
            .supported_output_configs()
            .expect("error while querying configs");

        let supported_config = supported_configs_range
            .next()
            .expect("no supported config?!")
            .with_sample_rate(cpal::SampleRate(16384 * 6));
        // but should
        // definitely look
        // into getting
        // the proper
        // sample rate

        let config = supported_config.into();
        let source = AudioSource::new(device, config, Vec::new(), ctx.address());
        self.sources.insert(source_name, source);

        Ok(AddSourceServerResponse {
            sources: self.sources.keys().map(|key| key.to_owned()).collect(),
        })
    }
}

impl Handler<ReadSourcesServerParams> for QueueServer {
    type Result = Result<ReadSourcesServerResponse, ErrorResponse>;

    fn handle(&mut self, _msg: ReadSourcesServerParams, _ctx: &mut Self::Context) -> Self::Result {
        Ok(ReadSourcesServerResponse {
            sources: self.sources.keys().map(|k| k.to_owned()).collect(),
        })
    }
}

impl Handler<SetAudioProgressServerParams> for QueueServer {
    type Result = Result<SetAudioProgressServerResponse, ErrorResponse>;
    fn handle(
        &mut self,
        msg: SetAudioProgressServerParams,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        info!("'SetAudioProgress' handler received a message, MESSAGE: {msg:?}");

        let SetAudioProgressServerParams {
            source_name,
            progress,
        } = msg;
        if let Some(source) = self.sources.get_mut(&source_name) {
            source.set_stream_progress(progress);
        } else {
            error!(
                "no audio source with the name {source_name} found, SOURCES: {:?}",
                self.sources.keys()
            );
            return Err(ErrorResponse {
                error: format!("no audio source with the name {source_name} found"),
            });
        }

        Ok(SetAudioProgressServerResponse)
    }
}

impl Handler<PauseQueueServerParams> for QueueServer {
    type Result = Result<PauseQueueServerResponse, ErrorResponse>;
    fn handle(&mut self, msg: PauseQueueServerParams, _ctx: &mut Self::Context) -> Self::Result {
        info!("'PauseQueue' handler received a message, MESSAGE: {msg:?}");

        let PauseQueueServerParams { source_name } = msg;
        if let Some(source) = self.sources.get_mut(&source_name) {
            source.set_stream_playback_state(crate::audio::PlaybackState::Paused)
        } else {
            error!(
                "no audio source with the name {source_name} found, SOURCES: {:?}",
                self.sources.keys()
            );
            return Err(ErrorResponse {
                error: format!("no audio source with the name {source_name} found"),
            });
        }

        Ok(PauseQueueServerResponse)
    }
}

impl Handler<UnPauseQueueServerParams> for QueueServer {
    type Result = Result<UnPauseQueueServerResponse, ErrorResponse>;
    fn handle(&mut self, msg: UnPauseQueueServerParams, _ctx: &mut Self::Context) -> Self::Result {
        info!("'UnPauseQueue' handler received a message, MESSAGE: {msg:?}");

        let UnPauseQueueServerParams { source_name } = msg;
        if let Some(source) = self.sources.get_mut(&source_name) {
            source.set_stream_playback_state(crate::audio::PlaybackState::Playing)
        } else {
            error!(
                "no audio source with the name {source_name} found, SOURCES: {:?}",
                self.sources.keys()
            );
            return Err(ErrorResponse {
                error: format!("no audio source with the name {source_name} found"),
            });
        }

        Ok(UnPauseQueueServerResponse)
    }
}

impl Handler<PlayNextServerParams> for QueueServer {
    type Result = Result<PlayNextServerResponse, ErrorResponse>;

    fn handle(&mut self, msg: PlayNextServerParams, _ctx: &mut Self::Context) -> Self::Result {
        info!("'PlayNext' handler received a message, MESSAGE: {msg:?}");

        let PlayNextServerParams { source_name } = msg.clone();

        if let Some(source) = self.sources.get_mut(&source_name) {
            if let Err(err) = source.play_next(source_name) {
                error!("failed to play next audio, MESSAGE: {msg:?}, ERROR: {err}");
                return Err(ErrorResponse {
                    error: format!("failed to play next audio, ERROR: {err}"),
                });
            }
        } else {
            error!(
                "no audio source with the name {source_name} found, SOURCES: {:?}",
                self.sources.keys()
            );
            return Err(ErrorResponse {
                error: format!("no audio source with the name {source_name} found"),
            });
        }

        Ok(PlayNextServerResponse)
    }
}

impl Handler<PlayPreviousServerParams> for QueueServer {
    type Result = Result<PlayPreviousServerResponse, ErrorResponse>;

    fn handle(&mut self, msg: PlayPreviousServerParams, _ctx: &mut Self::Context) -> Self::Result {
        info!("'PlayPrevious' handler received a message, MESSAGE: {msg:?}");

        let PlayPreviousServerParams { source_name } = msg.clone();

        if let Some(source) = self.sources.get_mut(&source_name) {
            if let Err(err) = source.play_prev(source_name) {
                error!("failed to play previous audio, MESSAGE: {msg:?}, ERROR: {err}");
                return Err(ErrorResponse {
                    error: format!("failed to play previous audio, ERROR: {err}"),
                });
            }
        } else {
            error!(
                "no audio source with the name {source_name} found, SOURCES: {:?}",
                self.sources.keys()
            );
            return Err(ErrorResponse {
                error: format!("no audio source with the name {source_name} found"),
            });
        }

        Ok(PlayPreviousServerResponse)
    }
}

impl Handler<PlaySelectedServerParams> for QueueServer {
    type Result = Result<PlaySelectedServerResponse, ErrorResponse>;

    fn handle(&mut self, msg: PlaySelectedServerParams, _ctx: &mut Self::Context) -> Self::Result {
        info!("'PlayPrevious' handler received a message, MESSAGE: {msg:?}");

        let PlaySelectedServerParams { index, source_name } = msg.clone();

        if let Some(source) = self.sources.get_mut(&source_name) {
            if let Err(err) = source.play_selected(index, source_name) {
                error!("failed to play selected audio, MESSAGE: {msg:?}, ERROR: {err}");
                return Err(ErrorResponse {
                    error: format!("failed to play track with index {index} audio, ERROR: {err}"),
                });
            }
        } else {
            error!(
                "no audio source with the name {source_name} found, SOURCES: {:?}",
                self.sources.keys()
            );
            return Err(ErrorResponse {
                error: format!("no audio source with the name {source_name} found"),
            });
        }

        Ok(PlaySelectedServerResponse)
    }
}

impl Handler<LoopQueueServerParams> for QueueServer {
    type Result = Result<LoopQueueServerResponse, ErrorResponse>;

    fn handle(&mut self, msg: LoopQueueServerParams, _ctx: &mut Self::Context) -> Self::Result {
        info!("'LoopQueue' handler received a message, MESSAGE: {msg:?}");

        let LoopQueueServerParams {
            source_name,
            bounds,
        } = msg;

        if let Some(source) = self.sources.get_mut(&source_name) {
            source.set_loop(bounds);
        } else {
            error!(
                "no audio source with the name {source_name} found, SOURCES: {:?}",
                self.sources.keys()
            );
            return Err(ErrorResponse {
                error: format!("no audio source/source with the name {source_name} found"),
            });
        }

        Ok(LoopQueueServerResponse)
    }
}
