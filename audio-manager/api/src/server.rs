use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::Path;

use actix::{Actor, ActorContext, Addr, AsyncContext, Context, Handler, Message, Recipient};

use log::{error, info};
use serde::{Deserialize, Serialize};

use crate::{
    audio::{AudioPlayer, PlaybackInfo, ProcessorInfo},
    downloader::{
        self, AudioDownloader, DownloadAudio, DownloadAudioResponse, NotifyDownloadFinished,
    },
    session::{AudioBrainSessionPassThroughMessages, FilteredPassThroughtMessage},
    utils::create_source,
    ErrorResponse, AUDIO_DIR, AUDIO_SOURCES,
};

pub struct AudioBrain {
    downloader_addr: Addr<AudioDownloader>,
    sources: HashMap<String, AudioPlayer>,
    sessions: HashMap<usize, Recipient<FilteredPassThroughtMessage>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AudioBrainMessage {
    AddQueueItem(AddQueueItemServerParams),
    RemoveQueueItem(RemoveQueueItemServerParams),
    ReadQueueItems(ReadAudioBrainParams),
    MoveQueueItem(MoveQueueItemServerParams),
    SetAudioProgress(SetAudioProgressServerParams),
    ReadSources(ReadSourcesServerParams),
    PauseQueue(PauseAudioBrainParams),
    UnPauseQueue(UnPauseAudioBrainParams),
    PlayNext(PlayNextServerParams),
    PlayPrevious(PlayPreviousServerParams),
    PlaySelected(PlaySelectedServerParams),
    LoopQueue(LoopAudioBrainParams),
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[allow(clippy::enum_variant_names, dead_code)]
pub enum AudioBrainMessageResponse {
    SessionConnectedResponse(ConnectServerResponse),
    AddQueueItemResponse(AddQueueItemServerResponse),
    RemoveQueueItemResponse(RemoveQueueItemServerResponse),
    FinishedDownloadingAudio(FinishedDownloadingAudioServerResponse),
    ReadQueueItemsResponse(ReadAudioBrainResponse),
    MoveQueueItemResponse(MoveQueueItemServerResponse),
    ReadSourcesResponse(ReadSourcesServerResponse),
    SetAudioProgress(SetAudioProgressServerResponse),
    ReadSources(ReadSourcesServerResponse),
    PauseQueueResponse(PauseAudioBrainResponse),
    UnPauseQueueResponse(UnPauseAudioBrainResponse),
    PlayNextResponse(PlayNextServerResponse),
    PlayPreviousResponse(PlayPreviousServerResponse),
    PlaySelectedResponse(PlaySelectedServerResponse),
    LoopQueueResponse(LoopAudioBrainResponse),
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
#[rtype(result = "Result<ReadAudioBrainResponse, ErrorResponse>")]
#[serde(rename_all = "camelCase")]
pub struct ReadAudioBrainParams {
    pub source_name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReadAudioBrainResponse {
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
#[rtype(result = "Result<PauseAudioBrainResponse, ErrorResponse>")]
#[serde(rename_all = "camelCase")]
pub struct PauseAudioBrainParams {
    pub source_name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PauseAudioBrainResponse;

#[derive(Debug, Clone, Deserialize, Message)]
#[rtype(result = "Result<UnPauseAudioBrainResponse, ErrorResponse>")]
#[serde(rename_all = "camelCase")]
pub struct UnPauseAudioBrainParams {
    pub source_name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnPauseAudioBrainResponse;

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
#[rtype(result = "Result<LoopAudioBrainResponse, ErrorResponse>")]
#[serde(rename_all = "camelCase")]
pub struct LoopAudioBrainParams {
    pub source_name: String,
    pub bounds: Option<LoopBounds>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoopAudioBrainResponse;

impl AudioBrain {
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

impl Actor for AudioBrain {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        // check this if weird shit happens just trying stuff here
        ctx.set_mailbox_capacity(64);
        info!("stared new 'AudioBrain', CONTEXT: {ctx:?}");

        for (human_readable_name, source_name) in AUDIO_SOURCES {
            let source = create_source(source_name, ctx.address());
            self.sources.insert(human_readable_name.to_owned(), source);
        }

        match self.downloader_addr.try_send(downloader::Connect {
            addr: ctx.address().into(),
        }) {
            Ok(_) => {}
            Err(err) => {
                error!("'AudioBrain' failed to connect to 'AudioDownloader', ERROR: {err}");
                ctx.stop();
            }
        };
    }
}

impl Handler<Connect> for AudioBrain {
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

impl Handler<Disconnect> for AudioBrain {
    type Result = ();
    fn handle(&mut self, msg: Disconnect, _ctx: &mut Self::Context) -> Self::Result {
        let Disconnect { id } = msg;
        self.sessions.remove(&id);
    }
}

impl Handler<SendClientQueueInfoParams> for AudioBrain {
    type Result = ();

    fn handle(&mut self, msg: SendClientQueueInfoParams, _ctx: &mut Self::Context) -> Self::Result {
        let SendClientQueueInfoParams {
            source_name,
            processor_info,
        } = msg;
        if let Some(source) = self.sources.get(&source_name) {
            for session in &self.sessions {
                let addr = session.1;
                let msg =
                    serde_json::to_string(&AudioBrainMessageResponse::SendClientQueueInfoResponse(
                        SendClientQueueInfoServerResponse {
                            playback_info: source.playback_info().clone(),
                            processor_info: processor_info.clone(),
                        },
                    ))
                    .unwrap_or(String::new());

                addr.do_send(FilteredPassThroughtMessage {
                    msg,
                    source_name: source_name.clone(),
                });
            }
        }
    }
}

impl Handler<AddQueueItemServerParams> for AudioBrain {
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
            AudioBrain::send_filtered_passthrought_msg(
                &sessions,
                serde_json::to_string(
                    &AudioBrainSessionPassThroughMessages::StartedDownloadingAudio,
                )
                .unwrap_or(String::new()),
                &source_name,
            );

            // TODO:
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

impl Handler<RemoveQueueItemServerParams> for AudioBrain {
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

impl Handler<NotifyDownloadFinished> for AudioBrain {
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

                AudioBrain::send_filtered_passthrought_msg(
                    &sessions,
                    serde_json::to_string(
                        &AudioBrainSessionPassThroughMessages::FinishedDownloadingAudio(resp),
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

                AudioBrain::send_filtered_passthrought_msg(
                    &sessions,
                    serde_json::to_string(
                        &AudioBrainSessionPassThroughMessages::FinishedDownloadingAudio(resp),
                    )
                    .unwrap_or(String::new()),
                    &err_resp.0,
                );
            }
        }
    }
}

impl Handler<ReadAudioBrainParams> for AudioBrain {
    type Result = Result<ReadAudioBrainResponse, ErrorResponse>;
    fn handle(&mut self, msg: ReadAudioBrainParams, _ctx: &mut Self::Context) -> Self::Result {
        info!("'ReadQueueItems' handler received a message, MESSAGE: {msg:?}");

        let ReadAudioBrainParams { source_name } = msg;
        if let Some(source) = self.sources.get(&source_name) {
            Ok(ReadAudioBrainResponse {
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

impl Handler<MoveQueueItemServerParams> for AudioBrain {
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

impl Handler<ReadSourcesServerParams> for AudioBrain {
    type Result = Result<ReadSourcesServerResponse, ErrorResponse>;

    fn handle(&mut self, _msg: ReadSourcesServerParams, _ctx: &mut Self::Context) -> Self::Result {
        Ok(ReadSourcesServerResponse {
            sources: self.sources.keys().map(|k| k.to_owned()).collect(),
        })
    }
}

impl Handler<SetAudioProgressServerParams> for AudioBrain {
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

impl Handler<PauseAudioBrainParams> for AudioBrain {
    type Result = Result<PauseAudioBrainResponse, ErrorResponse>;
    fn handle(&mut self, msg: PauseAudioBrainParams, _ctx: &mut Self::Context) -> Self::Result {
        info!("'PauseQueue' handler received a message, MESSAGE: {msg:?}");

        let PauseAudioBrainParams { source_name } = msg;
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

        Ok(PauseAudioBrainResponse)
    }
}

impl Handler<UnPauseAudioBrainParams> for AudioBrain {
    type Result = Result<UnPauseAudioBrainResponse, ErrorResponse>;
    fn handle(&mut self, msg: UnPauseAudioBrainParams, _ctx: &mut Self::Context) -> Self::Result {
        info!("'UnPauseQueue' handler received a message, MESSAGE: {msg:?}");

        let UnPauseAudioBrainParams { source_name } = msg;
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

        Ok(UnPauseAudioBrainResponse)
    }
}

impl Handler<PlayNextServerParams> for AudioBrain {
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

impl Handler<PlayPreviousServerParams> for AudioBrain {
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

impl Handler<PlaySelectedServerParams> for AudioBrain {
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

impl Handler<LoopAudioBrainParams> for AudioBrain {
    type Result = Result<LoopAudioBrainResponse, ErrorResponse>;

    fn handle(&mut self, msg: LoopAudioBrainParams, _ctx: &mut Self::Context) -> Self::Result {
        info!("'LoopQueue' handler received a message, MESSAGE: {msg:?}");

        let LoopAudioBrainParams {
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

        Ok(LoopAudioBrainResponse)
    }
}
