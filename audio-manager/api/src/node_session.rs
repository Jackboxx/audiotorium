use actix::{
    Actor, ActorContext, ActorFutureExt, Addr, AsyncContext, ContextFutureSpawner, Handler,
    Running, StreamHandler, WrapFuture,
};

use actix_web_actors::ws;
use log::{error, info};
use serde::{Deserialize, Serialize};

use crate::{
    audio::LoopBounds,
    node::{
        AddQueueItemNodeParams, AudioNode, LoopQueueNodeParams, MoveQueueItemNodeParams,
        NodeConnectMessage, NodeDisconnectMessage, NodeInternalMessage, NodeInternalResponse,
        NodeInternalUpdateMessage, PlaySelectedNodeParams, RemoveQueueItemNodeParams,
        SetAudioProgressNodeParams,
    },
    ErrorResponse,
};

pub struct AudioNodeSession {
    id: usize,
    node_addr: Addr<AudioNode>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NodeSessionWsMessage {
    AddQueueItem(AddQueueItemNodeSessionParams),
    RemoveQueueItem(RemoveQueueItemNodeSessionParams),
    ReadQueueItems,
    MoveQueueItem(MoveQueueItemNodeSessionParams),
    SetAudioProgress(SetAudioProgressNodeSessionParams),
    PauseQueue,
    UnPauseQueue,
    PlayNext,
    PlayPrevious,
    PlaySelected(PlaySelectedNodeSessionParams),
    LoopQueue(LoopNodeSessionParams),
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NodeSessionWsResponse {
    SessionConnectedResponse(Vec<String>),
    ErrorResponse(ErrorResponse),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddQueueItemNodeSessionParams {
    pub title: String,
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveQueueItemNodeSessionParams {
    pub index: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaySelectedNodeSessionParams {
    pub index: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveQueueItemNodeSessionParams {
    pub old_pos: usize,
    pub new_pos: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetAudioProgressNodeSessionParams {
    pub progress: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoopNodeSessionParams {
    pub bounds: Option<LoopBounds>,
}

impl Into<NodeInternalMessage> for NodeSessionWsMessage {
    fn into(self) -> NodeInternalMessage {
        match self {
            Self::AddQueueItem(AddQueueItemNodeSessionParams { title, url }) => {
                NodeInternalMessage::AddQueueItem(AddQueueItemNodeParams { title, url })
            }
            Self::RemoveQueueItem(RemoveQueueItemNodeSessionParams { index }) => {
                NodeInternalMessage::RemoveQueueItem(RemoveQueueItemNodeParams { index })
            }
            Self::ReadQueueItems => NodeInternalMessage::ReadQueueItems,
            Self::MoveQueueItem(MoveQueueItemNodeSessionParams { old_pos, new_pos }) => {
                NodeInternalMessage::MoveQueueItem(MoveQueueItemNodeParams { old_pos, new_pos })
            }
            Self::SetAudioProgress(SetAudioProgressNodeSessionParams { progress }) => {
                NodeInternalMessage::SetAudioProgress(SetAudioProgressNodeParams { progress })
            }
            Self::PauseQueue => NodeInternalMessage::PauseQueue,
            Self::UnPauseQueue => NodeInternalMessage::UnPauseQueue,
            Self::PlayNext => NodeInternalMessage::PlayNext,
            Self::PlayPrevious => NodeInternalMessage::PlayPrevious,
            Self::PlaySelected(PlaySelectedNodeSessionParams { index }) => {
                NodeInternalMessage::PlaySelected(PlaySelectedNodeParams { index })
            }
            Self::LoopQueue(LoopNodeSessionParams { bounds }) => {
                NodeInternalMessage::LoopQueue(LoopQueueNodeParams { bounds })
            }
        }
    }
}

impl Actor for AudioNodeSession {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        info!("stared new 'NodSession'");

        let addr = ctx.address();
        self.node_addr
            .send(NodeConnectMessage { addr })
            .into_actor(self)
            .then(|res, act, ctx| {
                match res {
                    Ok(res) => {
                        info!("'NodeSession' connected");
                        act.id = res.id;

                        ctx.text(
                            serde_json::to_string(
                                &NodeSessionWsResponse::SessionConnectedResponse(res.queue),
                            )
                            .unwrap_or("[]".to_owned()),
                        );
                    }

                    Err(err) => {
                        error!("'NodeSession' failed to connect to 'AudioNode', ERROR: {err}");
                        ctx.stop();
                    }
                }

                actix::fut::ready(())
            })
            .wait(ctx);
    }

    fn stopping(&mut self, _: &mut Self::Context) -> Running {
        info!("'AudioNodeSession' stopping, ID: {}", self.id);

        self.node_addr
            .do_send(NodeDisconnectMessage { id: self.id });

        Running::Stop
    }
}

impl Handler<NodeInternalResponse> for AudioNodeSession {
    type Result = ();

    /// used to receive multicast messages from nodes
    fn handle(&mut self, msg: NodeInternalResponse, ctx: &mut Self::Context) -> Self::Result {
        ctx.text(serde_json::to_string(&msg).unwrap_or(String::from("{}")))
    }
}

impl Handler<NodeInternalUpdateMessage> for AudioNodeSession {
    type Result = ();

    /// used to receive multicast messages from nodes
    fn handle(&mut self, msg: NodeInternalUpdateMessage, ctx: &mut Self::Context) -> Self::Result {
        ctx.text(serde_json::to_string(&msg).unwrap_or(String::from("{}")))
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for AudioNodeSession {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match &msg {
            Ok(ws::Message::Text(text)) => {
                match serde_json::from_str::<NodeSessionWsMessage>(text) {
                    Ok(msg) => {
                        let addr = self.node_addr.clone();
                        let node_msg: NodeInternalMessage = msg.clone().into();
                        let fut = async move { addr.send(node_msg).await }.into_actor(self).map(
                            move |result, _, ctx| match result {
                                Ok(resp) => {
                                    ctx.text(serde_json::to_string(&resp).unwrap_or(String::from("{}")))
                                }
                                Err(err) => {
                                    error!("audio node didn't responde to message '{msg:?}', ERROR: {err}",);
                                    ctx.text(
                                        serde_json::to_string(
                                            &NodeSessionWsResponse::ErrorResponse(ErrorResponse {
                                                error: format!("server failed to responde to message, ERROR: {err}"),
                                            }),
                                        )
                                        .unwrap_or(String::from("{}")),
                                    );
                                }
                            },
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
