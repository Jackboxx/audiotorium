use actix::{
    Actor, ActorContext, ActorFutureExt, Addr, AsyncContext, ContextFutureSpawner, Handler,
    Message, Running, StreamHandler, WrapFuture,
};
use actix_web_actors::ws;
use log::{error, info};
use serde::{Deserialize, Serialize};

use crate::{
    server::{
        AddQueueItemServerParams, Connect, Disconnect, FinishedDownloadingAudioServerResponse,
        LoopBounds, LoopQueueServerParams, MoveQueueItemServerParams, PauseQueueServerParams,
        PlayNextServerParams, PlayPreviousServerParams, PlaySelectedServerParams, QueueServer,
        QueueServerMessageResponse, ReadQueueServerParams, ReadSourcesServerParams,
        ReadSourcesServerResponse, RemoveQueueItemServerParams, SetAudioProgressServerParams,
        UnPauseQueueServerParams,
    },
    ErrorResponse,
};

macro_rules! send_and_handle_queue_server_msg {
    ($enum:ident::$variant:ident, $msg_name: literal, $actor: tt, $msg: tt) => {{
        let addr = $actor.server_addr.clone();
        async move { addr.send($msg).await }
            .into_actor($actor)
            .map(|result, _, ctx| match result {
                Ok(resp) => match resp {
                    Ok(res) => {
                        ctx.text(
                            serde_json::to_string(
                                &$crate::server::QueueServerMessageResponse::$variant(res),
                            )
                            .unwrap_or("{}".to_owned()),
                        );
                    }
                    Err(err_resp) => {
                        ctx.text(
                            serde_json::to_string(&QueueSessionResponse::ErrorResponse(err_resp))
                                .unwrap_or("{}".to_owned()),
                        );
                    }
                },
                Err(err) => {
                    error!(
                        "queue server didn't responde to message '{}', ERROR: {err}",
                        $msg_name
                    );
                    ctx.text(
                        serde_json::to_string(&QueueSessionResponse::ErrorResponse(
                            ErrorResponse {
                                error: format!(
                                    "server failed to responde to message '{}', ERROR: {err}",
                                    $msg_name
                                ),
                            },
                        ))
                        .unwrap_or("{}".to_owned()),
                    );
                }
            })
    }};
}

#[derive(Debug, Clone)]
pub struct QueueSession {
    id: usize,
    active_source_name: String,
    server_addr: Addr<QueueServer>,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct FilteredPassThroughtMessage {
    pub msg: String,
    pub source_name: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum QueueSessionMessage {
    SetActiveSource(SetActiveSourceSessionParams),
    AddQueueItem(AddQueueItemSessionParams),
    RemoveQueueItem(RemoveQueueItemSessionParams),
    ReadQueueItems,
    MoveQueueItem(MoveQueueItemSessionParams),
    SetAudioProgress(SetAudioProgressSessionParams),
    PauseQueue,
    UnPauseQueue,
    PlayNext,
    PlayPrevious,
    PlaySelected(PlaySelectedSessionParams),
    LoopQueue(LoopQueueSessionParams),
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[allow(clippy::enum_variant_names)]
pub enum QueueSessionResponse {
    SetActiveSourceResponse(SetActiveSourceSessionResponse),
    ErrorResponse(ErrorResponse),
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum QueueSessionPassThroughMessages {
    StartedDownloadingAudio,
    FinishedDownloadingAudio(FinishedDownloadingAudioServerResponse),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetActiveSourceSessionParams {
    pub source_name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetActiveSourceSessionResponse {
    pub source_name: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddQueueItemSessionParams {
    pub title: String,
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveQueueItemSessionParams {
    pub index: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaySelectedSessionParams {
    pub index: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveQueueItemSessionParams {
    old_pos: usize,
    new_pos: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetAudioProgressSessionParams {
    pub progress: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoopQueueSessionParams {
    pub bounds: Option<LoopBounds>,
}

impl QueueSession {
    pub fn new(server_addr: Addr<QueueServer>) -> Self {
        Self {
            id: usize::MAX,
            active_source_name: String::new(),
            server_addr,
        }
    }
}

impl Actor for QueueSession {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        info!("stared new 'QueueSession'");

        let addr = ctx.address();
        self.server_addr
            .send(Connect {
                addr: addr.recipient(),
            })
            .into_actor(self)
            .then(|res, act, ctx| {
                match res {
                    Ok(res) => match res {
                        Ok(params) => {
                            info!("'QueueSession' connected");
                            act.id = params.id;

                            ctx.text(
                                serde_json::to_string(
                                    &QueueServerMessageResponse::SessionConnectedResponse(params),
                                )
                                .unwrap_or("[]".to_owned()),
                            );
                        }
                        Err(err) => {
                            error!(
                                "'QueueSession' failed to connect to 'QueueServer', ERROR: {err:?}"
                            );
                            ctx.stop();
                        }
                    },

                    Err(err) => {
                        error!("'QueueSession' failed to connect to 'QueueServer', ERROR: {err}");
                        ctx.stop();
                    }
                }

                actix::fut::ready(())
            })
            .wait(ctx);
    }

    fn stopping(&mut self, _: &mut Self::Context) -> Running {
        info!("'QueueSession' stopping, ID: {}", self.id);

        self.server_addr.do_send(Disconnect { id: self.id });
        Running::Stop
    }
}

impl Handler<FilteredPassThroughtMessage> for QueueSession {
    type Result = ();
    fn handle(
        &mut self,
        msg: FilteredPassThroughtMessage,
        ctx: &mut Self::Context,
    ) -> Self::Result {
        let FilteredPassThroughtMessage { msg, source_name } = msg;

        if source_name == self.active_source_name {
            ctx.text(msg);
        }
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for QueueSession {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match &msg {
            Ok(ws::Message::Text(text)) => {
                match serde_json::from_str::<QueueSessionMessage>(text) {
                    Ok(QueueSessionMessage::SetActiveSource(params)) => {
                        let msg = ReadSourcesServerParams {};

                        let addr = self.server_addr.clone();

                        let fut = async move { addr.send(msg).await }.into_actor(self).map(
                            |result, act, ctx| match result {
                                Ok(resp) => match resp {
                                    Ok(ReadSourcesServerResponse { sources }) => {
                                        if sources.contains(&params.source_name) {
                                            act.active_source_name = params.source_name;
                                        };

                                        ctx.text(
                                            serde_json::to_string(
                                                &QueueSessionResponse::SetActiveSourceResponse(
                                                    SetActiveSourceSessionResponse {
                                                        source_name: act.active_source_name.clone(),
                                                    },
                                                ),
                                            )
                                            .unwrap_or("{}".to_owned()),
                                        );
                                    }
                                    Err(err_resp) => {
                                        ctx.text(
                                            serde_json::to_string(&err_resp)
                                                .unwrap_or("{}".to_owned()),
                                        );
                                    }
                                },
                                Err(err) => {
                                    error!("queue server didn't responde to message, ERROR: {err}");
                                    ctx.text(
                        serde_json::to_string(&ErrorResponse {
                            error: format!("server failed to responde to message, ERROR: {err}"),
                        })
                        .unwrap_or("{}".to_owned()),
                    );
                                }
                            },
                        );

                        ctx.spawn(fut);
                    }
                    Ok(QueueSessionMessage::AddQueueItem(params)) => {
                        let msg = AddQueueItemServerParams {
                            source_name: self.active_source_name.clone(),
                            title: params.title,
                            url: params.url,
                        };

                        let fut = send_and_handle_queue_server_msg!(
                            QueueServerMessageResponse::AddQueueItemResponse,
                            "AddQueueItemResponse",
                            self,
                            msg
                        );

                        ctx.spawn(fut);
                    }
                    Ok(QueueSessionMessage::RemoveQueueItem(params)) => {
                        let msg = RemoveQueueItemServerParams {
                            source_name: self.active_source_name.clone(),
                            index: params.index,
                        };

                        let fut = send_and_handle_queue_server_msg!(
                            QueueServerMessageResponse::RemoveQueueItemResponse,
                            "RemoveQueueItemResponse",
                            self,
                            msg
                        );

                        ctx.spawn(fut);
                    }
                    Ok(QueueSessionMessage::ReadQueueItems) => {
                        let msg = ReadQueueServerParams {
                            source_name: self.active_source_name.clone(),
                        };

                        let fut = send_and_handle_queue_server_msg!(
                            QueueServerMessageResponse::ReadQueueItemsResponse,
                            "ReadQueueItemsResponse",
                            self,
                            msg
                        );

                        ctx.spawn(fut);
                    }
                    Ok(QueueSessionMessage::MoveQueueItem(params)) => {
                        let msg = MoveQueueItemServerParams {
                            source_name: self.active_source_name.clone(),
                            old_pos: params.old_pos,
                            new_pos: params.new_pos,
                        };

                        let fut = send_and_handle_queue_server_msg!(
                            QueueServerMessageResponse::MoveQueueItemResponse,
                            "MoveQueueItemResponse",
                            self,
                            msg
                        );

                        ctx.spawn(fut);
                    }
                    Ok(QueueSessionMessage::SetAudioProgress(params)) => {
                        let msg = SetAudioProgressServerParams {
                            source_name: self.active_source_name.clone(),
                            progress: params.progress,
                        };

                        let fut = send_and_handle_queue_server_msg!(
                            QueueServerMessageResponse::SetAudioProgress,
                            "SetAudioProgress",
                            self,
                            msg
                        );

                        ctx.spawn(fut);
                    }
                    Ok(QueueSessionMessage::PauseQueue) => {
                        let msg = PauseQueueServerParams {
                            source_name: self.active_source_name.clone(),
                        };

                        let fut = send_and_handle_queue_server_msg!(
                            QueueServerMessageResponse::PauseQueueResponse,
                            "PauseQueueResponse",
                            self,
                            msg
                        );

                        ctx.spawn(fut);
                    }
                    Ok(QueueSessionMessage::UnPauseQueue) => {
                        let msg = UnPauseQueueServerParams {
                            source_name: self.active_source_name.clone(),
                        };

                        let fut = send_and_handle_queue_server_msg!(
                            QueueServerMessageResponse::UnPauseQueueResponse,
                            "UnPauseQueueResponse",
                            self,
                            msg
                        );

                        ctx.spawn(fut);
                    }
                    Ok(QueueSessionMessage::PlayNext) => {
                        let msg = PlayNextServerParams {
                            source_name: self.active_source_name.clone(),
                        };

                        let fut = send_and_handle_queue_server_msg!(
                            QueueServerMessageResponse::PlayNextResponse,
                            "PlayNextResponse",
                            self,
                            msg
                        );

                        ctx.spawn(fut);
                    }
                    Ok(QueueSessionMessage::PlayPrevious) => {
                        let msg = PlayPreviousServerParams {
                            source_name: self.active_source_name.clone(),
                        };

                        let fut = send_and_handle_queue_server_msg!(
                            QueueServerMessageResponse::PlayPreviousResponse,
                            "PlayPreviousResponse",
                            self,
                            msg
                        );

                        ctx.spawn(fut);
                    }
                    Ok(QueueSessionMessage::PlaySelected(params)) => {
                        let msg = PlaySelectedServerParams {
                            source_name: self.active_source_name.clone(),
                            index: params.index,
                        };

                        let fut = send_and_handle_queue_server_msg!(
                            QueueServerMessageResponse::PlaySelectedResponse,
                            "PlaySelectedResponse",
                            self,
                            msg
                        );

                        ctx.spawn(fut);
                    }
                    Ok(QueueSessionMessage::LoopQueue(params)) => {
                        let msg = LoopQueueServerParams {
                            source_name: self.active_source_name.clone(),
                            bounds: params.bounds,
                        };

                        let fut = send_and_handle_queue_server_msg!(
                            QueueServerMessageResponse::LoopQueueResponse,
                            "LoopQueueResponse",
                            self,
                            msg
                        );

                        ctx.spawn(fut);
                    }
                    Err(err) => {
                        error!("failed to parse message, MESSAGE: {msg:?}, ERROR: {err}");

                        ctx.text(
                            serde_json::to_string(&ErrorResponse {
                                error: format!("failed to parse message, ERROR: {err}"),
                            })
                            .unwrap_or(String::from("{}")),
                        )
                    }
                }
            }
            Ok(ws::Message::Close(reason)) => {
                ctx.close(reason.clone());
                ctx.stop();
            }
            _ => {}
        }
    }
}
