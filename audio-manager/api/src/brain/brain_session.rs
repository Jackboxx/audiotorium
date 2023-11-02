use actix::{
    Actor, ActorContext, ActorFutureExt, Addr, AsyncContext, ContextFutureSpawner, Handler,
    Running, StreamHandler, WrapFuture,
};

use actix_web_actors::ws;
use serde::Serialize;

use crate::{
    brain::brain_server::{BrainConnectMessage, BrainDisconnect},
    node::node_server::AudioNodeInfo,
    streams::brain_streams::{
        get_type_of_stream_data, AudioBrainInfoStreamMessage, AudioBrainInfoStreamType,
    },
};

use super::brain_server::AudioBrain;

#[derive(Debug, Clone)]
pub struct AudioBrainSession {
    id: usize,
    server_addr: Addr<AudioBrain>,
    wanted_info: Vec<AudioBrainInfoStreamType>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BrainSessionWsResponse {
    SessionConnectedResponse {
        node_info: Option<Vec<AudioNodeInfo>>,
    },
}

impl AudioBrainSession {
    pub fn new(server_addr: Addr<AudioBrain>, wanted_info: Vec<AudioBrainInfoStreamType>) -> Self {
        Self {
            id: usize::MAX,
            server_addr,
            wanted_info,
        }
    }
}

impl Actor for AudioBrainSession {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        log::info!("stared new 'AudioBrainSession'");

        let addr = ctx.address();
        self.server_addr
            .send(BrainConnectMessage {
                addr,
                wanted_info: self.wanted_info.clone(),
            })
            .into_actor(self)
            .then(|res, act, ctx| {
                match res {
                    Ok(res) => {
                        log::info!("'AudioBrainSession' connected");
                        act.id = res.id;

                        ctx.text(
                            serde_json::to_string(&res.connection_response)
                                .unwrap_or("[]".to_owned()),
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

impl Handler<AudioBrainInfoStreamMessage> for AudioBrainSession {
    type Result = ();

    /// used to receive multicast messages from nodes
    fn handle(
        &mut self,
        msg: AudioBrainInfoStreamMessage,
        ctx: &mut Self::Context,
    ) -> Self::Result {
        let msg_type = get_type_of_stream_data(&msg);

        if self.wanted_info.contains(&msg_type) {
            ctx.text(serde_json::to_string(&msg).unwrap_or(String::from("{}")))
        }
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
