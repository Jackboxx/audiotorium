use actix::{
    Actor, ActorContext, ActorFutureExt, Addr, AsyncContext, ContextFutureSpawner, Handler,
    Message, Running, StreamHandler, WrapFuture,
};
use actix_web_actors::ws;
use log::{error, info};

use crate::{
    server::{Connect, Disconnect, QueueServer, QueueServerMessage, QueueServerMessageResponse},
    ErrorResponse,
};

#[derive(Debug, Clone)]
pub struct QueueSession {
    server_addr: Addr<QueueServer>,
    id: usize,
}

#[derive(Debug, Clone, Message)]
#[rtype(result = "()")]
pub struct PassThroughtMessage(pub String);

impl QueueSession {
    pub fn new(server_addr: Addr<QueueServer>) -> Self {
        Self {
            server_addr,
            id: usize::MAX,
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
                    Ok(res) => {
                        info!("'QueueSession' connected, ID: {res}");
                        act.id = res
                    }
                    Err(err) => {
                        error!("'QueueSession' failed to connect to 'QueueServer', ERROR: {err}");
                        ctx.stop()
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

impl Handler<PassThroughtMessage> for QueueSession {
    type Result = ();
    fn handle(&mut self, msg: PassThroughtMessage, ctx: &mut Self::Context) -> Self::Result {
        ctx.text(msg.0);
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for QueueSession {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match &msg {
            Ok(ws::Message::Text(text)) => match serde_json::from_str::<QueueServerMessage>(text) {
                Ok(QueueServerMessage::AddQueueItem(msg)) => {
                    let addr = self.server_addr.clone();
                    let fut = async move {
                            addr.send(msg).await
                        }.into_actor(self).map(|result, _, ctx| {
                            match result {
                                Ok(resp) => match resp {
                                    Ok(params) => {
                                        ctx.text(serde_json::to_string(&QueueServerMessageResponse::AddQueueItemResponse(params)).unwrap_or("[]".to_owned()));
                                    }
                                    Err(err_resp) => {
                                        ctx.text(serde_json::to_string(&err_resp).unwrap_or("{}".to_owned()));
                                    }
                                },
                                Err(err) => {
                                    error!("queue server didn't responde to 'AddQueueItem' message, ERROR: {err}");
                                    ctx.text(serde_json::to_string(&ErrorResponse {
                                        error: format!("server failed to responde to message, ERROR: {err}")
                                    }).unwrap_or("{}".to_owned()));
                                }
                            }
                        });

                    ctx.spawn(fut);
                }
                Ok(QueueServerMessage::AddSource(msg)) => {
                    let addr = self.server_addr.clone();
                    let fut = async move {
                            addr.send(msg).await
                        }.into_actor(self).map(|result, _, ctx| {
                            match result {
                                Ok(resp) => match resp {
                                    Ok(params) => {
                                        ctx.text(serde_json::to_string(&QueueServerMessageResponse::AddSourceResponse(params)).unwrap_or("[]".to_owned()));
                                    }
                                    Err(err_resp) => {
                                        ctx.text(serde_json::to_string(&err_resp).unwrap_or("{}".to_owned()));
                                    }
                                },
                                Err(err) => {
                                    error!("queue server didn't responde to 'AddSource' message, ERROR: {err}");
                                    ctx.text(serde_json::to_string(&ErrorResponse {
                                        error: format!("server failed to responde to message, ERROR: {err}")
                                    }).unwrap_or("{}".to_owned()));
                                }
                            }
                        });

                    ctx.spawn(fut);
                }
                Ok(QueueServerMessage::PlayNext(msg)) => {
                    let addr = self.server_addr.clone();
                    let fut = async move {
                            addr.send(msg).await
                        }.into_actor(self).map(|result, _, ctx| {
                            match result {
                                Ok(resp) => match resp {
                                    Ok(params) => {
                                        ctx.text(serde_json::to_string(&QueueServerMessageResponse::PlayNextResponse(params)).unwrap_or("[]".to_owned()));
                                    }
                                    Err(err_resp) => {
                                        ctx.text(serde_json::to_string(&err_resp).unwrap_or("{}".to_owned()));
                                    }
                                },
                                Err(err) => {
                                    error!("queue server didn't responde to 'PlayNext' message, ERROR: {err}");
                                    ctx.text(serde_json::to_string(&ErrorResponse {
                                        error: format!("server failed to responde to message, ERROR: {err}")
                                    }).unwrap_or("{}".to_owned()));
                                }
                            }
                        });

                    ctx.spawn(fut);
                }
                Ok(QueueServerMessage::PlayPrevious(msg)) => {
                    let addr = self.server_addr.clone();
                    let fut = async move {
                            addr.send(msg).await
                        }.into_actor(self).map(|result, _, ctx| {
                            match result {
                                Ok(resp) => match resp {
                                    Ok(params) => {
                                        ctx.text(serde_json::to_string(&QueueServerMessageResponse::PlayPreviousResponse(params)).unwrap_or("[]".to_owned()));
                                    }
                                    Err(err_resp) => {
                                        ctx.text(serde_json::to_string(&err_resp).unwrap_or("{}".to_owned()));
                                    }
                                },
                                Err(err) => {
                                    error!("queue server didn't responde to 'PlayPrevious' message, ERROR: {err}");
                                    ctx.text(serde_json::to_string(&ErrorResponse {
                                        error: format!("server failed to responde to message, ERROR: {err}")
                                    }).unwrap_or("{}".to_owned()));
                                }
                            }
                        });

                    ctx.spawn(fut);
                }
                Ok(QueueServerMessage::PlaySelected(msg)) => {
                    let addr = self.server_addr.clone();
                    let fut = async move {
                            addr.send(msg).await
                        }.into_actor(self).map(|result, _, ctx| {
                            match result {
                                Ok(resp) => match resp {
                                    Ok(params) => {
                                        ctx.text(serde_json::to_string(&QueueServerMessageResponse::PlaySelectedResponse(params)).unwrap_or("[]".to_owned()));
                                    }
                                    Err(err_resp) => {
                                        ctx.text(serde_json::to_string(&err_resp).unwrap_or("{}".to_owned()));
                                    }
                                },
                                Err(err) => {
                                    error!("queue server didn't responde to 'PlaySelected' message, ERROR: {err}");
                                    ctx.text(serde_json::to_string(&ErrorResponse {
                                        error: format!("server failed to responde to message, ERROR: {err}")
                                    }).unwrap_or("{}".to_owned()));
                                }
                            }
                        });

                    ctx.spawn(fut);
                }
                Ok(QueueServerMessage::LoopQueue(msg)) => {
                    let addr = self.server_addr.clone();
                    let fut = async move {
                            addr.send(msg).await
                        }.into_actor(self).map(|result, _, ctx| {
                            match result {
                                Ok(resp) => match resp {
                                    Ok(params) => {
                                        ctx.text(serde_json::to_string(&QueueServerMessageResponse::LoopQueueResponse(params)).unwrap_or("[]".to_owned()));
                                    }
                                    Err(err_resp) => {
                                        ctx.text(serde_json::to_string(&err_resp).unwrap_or("{}".to_owned()));
                                    }
                                },
                                Err(err) => {
                                    error!("queue server didn't responde to 'PlayNext' message, ERROR: {err}");
                                    ctx.text(serde_json::to_string(&ErrorResponse {
                                        error: format!("server failed to responde to message, ERROR: {err}")
                                    }).unwrap_or("{}".to_owned()));
                                }
                            }
                        });

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
            },
            Ok(ws::Message::Close(reason)) => {
                ctx.close(reason.clone());
                ctx.stop();
            }
            _ => {}
        }
    }
}
