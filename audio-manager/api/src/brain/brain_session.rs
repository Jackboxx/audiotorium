use actix::{
    Actor, ActorContext, ActorFutureExt, Addr, AsyncContext, ContextFutureSpawner, Handler,
    Message, Running, StreamHandler, WrapFuture,
};

use actix_web_actors::ws;
use serde::Serialize;

use crate::{
    brain::brain_server::{BrainConnect, BrainDisconnect},
    node::node_server::AudioNodeInfo,
};

use super::brain_server::AudioBrain;

#[derive(Debug, Clone)]
pub struct AudioBrainSession {
    id: usize,
    server_addr: Addr<AudioBrain>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[allow(clippy::enum_variant_names)]
pub enum AudioBrainSessionResponse {
    SessionConnectedResponse(Vec<AudioNodeInfo>),
}

#[derive(Debug, Clone, Serialize, Message)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[rtype(result = "()")]
#[allow(clippy::enum_variant_names)]
pub enum AudioBrainSessionInternalUpdateMessage {
    NodeInformationUpdate(Vec<AudioNodeInfo>),
}

impl AudioBrainSession {
    pub fn new(server_addr: Addr<AudioBrain>) -> Self {
        Self {
            id: usize::MAX,
            server_addr,
        }
    }
}

impl Actor for AudioBrainSession {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        log::info!("stared new 'AudioBrainSession'");

        let addr = ctx.address();
        self.server_addr
            .send(BrainConnect { addr })
            .into_actor(self)
            .then(|res, act, ctx| {
                match res {
                    Ok(params) => {
                        log::info!("'AudioBrainSession' connected");
                        act.id = params.id;

                        ctx.text(
                            serde_json::to_string(
                                &AudioBrainSessionResponse::SessionConnectedResponse(
                                    params.sources,
                                ),
                            )
                            .unwrap_or(String::from("[]")),
                        );
                    }

                    Err(err) => {
                        log::error!(
                            "'AudioBrainSession' failed to connect to 'AudioBrain', ERROR: {err}"
                        );
                        ctx.stop();
                    }
                }

                actix::fut::ready(())
            })
            .wait(ctx);
    }

    fn stopping(&mut self, _: &mut Self::Context) -> Running {
        log::info!("'AudioBrainSession' stopping, ID: {}", self.id);

        self.server_addr.do_send(BrainDisconnect { id: self.id });
        Running::Stop
    }
}

impl Handler<AudioBrainSessionInternalUpdateMessage> for AudioBrainSession {
    type Result = ();

    /// used to receive multicast messages from nodes
    fn handle(
        &mut self,
        msg: AudioBrainSessionInternalUpdateMessage,
        ctx: &mut Self::Context,
    ) -> Self::Result {
        ctx.text(serde_json::to_string(&msg).unwrap_or(String::from("{}")))
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for AudioBrainSession {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match &msg {
            Ok(ws::Message::Text(_text)) => {}
            Ok(ws::Message::Close(reason)) => {
                ctx.close(reason.clone());
                ctx.stop();
            }
            _ => {}
        }
    }
}
