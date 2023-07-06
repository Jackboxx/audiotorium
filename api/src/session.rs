use actix::{Actor, ActorContext, ActorFutureExt, Addr, AsyncContext, StreamHandler, WrapFuture};
use actix_web_actors::ws;
use log::{error, info};

use crate::{
    server::{QueueMessage, QueueServer},
    ErrorResponse,
};

#[derive(Debug, Clone)]
pub struct QueueSession {
    server_addr: Addr<QueueServer>,
}

impl QueueSession {
    pub fn new(server_addr: Addr<QueueServer>) -> Self {
        Self { server_addr }
    }
}

impl Actor for QueueSession {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        info!("stared new 'QueueSession'");
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for QueueSession {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match &msg {
            Ok(ws::Message::Text(text)) => match serde_json::from_str::<QueueMessage>(text) {
                Ok(QueueMessage::AddQueueItem(msg)) => {
                    let addr = self.server_addr.clone();
                    let fut = async move {
                            addr.send(msg).await
                        }.into_actor(self).map(|result, _, ctx| {
                            match result {
                                Ok(resp) => match resp {
                                    Ok(queue) => {
                                        ctx.text(serde_json::to_string(&queue).unwrap_or("[]".to_owned()));
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
                Ok(QueueMessage::AddSource(msg)) => {
                    let addr = self.server_addr.clone();
                    let fut = async move {
                            addr.send(msg).await
                        }.into_actor(self).map(|result, _, ctx| {
                            match result {
                                Ok(resp) => match resp {
                                    Ok(sources) => {
                                        ctx.text(serde_json::to_string(&sources).unwrap_or("[]".to_owned()));
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
                Ok(QueueMessage::PlayNext(msg)) => {
                    let addr = self.server_addr.clone();
                    let fut = async move {
                            addr.send(msg).await
                        }.into_actor(self).map(|result, _, ctx| {
                            match result {
                                Ok(resp) => match resp {
                                    Ok(()) => {}
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
                Ok(QueueMessage::PlayPrevious(msg)) => {
                    let addr = self.server_addr.clone();
                    let fut = async move {
                            addr.send(msg).await
                        }.into_actor(self).map(|result, _, ctx| {
                            match result {
                                Ok(resp) => match resp {
                                    Ok(()) => {}
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
                Ok(QueueMessage::PlaySelected(msg)) => {
                    let addr = self.server_addr.clone();
                    let fut = async move {
                            addr.send(msg).await
                        }.into_actor(self).map(|result, _, ctx| {
                            match result {
                                Ok(resp) => match resp {
                                    Ok(()) => {}
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
                Ok(QueueMessage::LoopQueue(msg)) => {
                    let addr = self.server_addr.clone();
                    let fut = async move {
                            addr.send(msg).await
                        }.into_actor(self).map(|result, _, ctx| {
                            match result {
                                Ok(resp) => match resp {
                                    Ok(()) => {}
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
