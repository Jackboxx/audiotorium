use actix::{
    Actor, ActorContext, ActorFutureExt, Addr, AsyncContext, ContextFutureSpawner, Handler,
    Running, StreamHandler, WrapFuture,
};

use actix_web_actors::ws;
use log::{error, info};
use serde::Serialize;
use ts_rs::TS;

use crate::{
    audio::audio_item::AudioMetaData,
    node::node_server::{NodeConnectMessage, NodeDisconnectMessage},
    streams::node_streams::{
        get_type_of_stream_data, AudioNodeInfoStreamMessage, AudioNodeInfoStreamType,
    },
};

use super::node_server::{AudioNode, AudioNodeHealth};

pub struct AudioNodeSession {
    id: usize,
    node_addr: Addr<AudioNode>,
    wanted_info: Vec<AudioNodeInfoStreamType>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[ts(export, export_to = "../app/src/api-types/")]
pub enum NodeSessionWsResponse {
    SessionConnectedResponse {
        // can't use SerializableQueue due to issue discussed
        // here: https://github.com/Aleph-Alpha/ts-rs/issues/70
        queue: Option<Vec<AudioMetaData>>,
        health: Option<AudioNodeHealth>,
    },
}

impl AudioNodeSession {
    pub fn new(node_addr: Addr<AudioNode>, wanted_info: Vec<AudioNodeInfoStreamType>) -> Self {
        Self {
            id: usize::MAX,
            node_addr,
            wanted_info,
        }
    }
}

impl Actor for AudioNodeSession {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        info!("stared new 'NodSession'");

        let addr = ctx.address();
        self.node_addr
            .send(NodeConnectMessage {
                addr,
                wanted_info: self.wanted_info.clone(),
            })
            .into_actor(self)
            .then(|res, act, ctx| {
                match res {
                    Ok(res) => {
                        info!("'NodeSession' connected");
                        act.id = res.id;

                        ctx.text(
                            serde_json::to_string(&res.connection_response)
                                .unwrap_or("failed to serialize on server".to_owned()),
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

impl Handler<AudioNodeInfoStreamMessage> for AudioNodeSession {
    type Result = ();

    /// used to receive multicast messages from nodes
    fn handle(&mut self, msg: AudioNodeInfoStreamMessage, ctx: &mut Self::Context) -> Self::Result {
        let msg_type = get_type_of_stream_data(&msg);

        if self.wanted_info.contains(&msg_type) {
            ctx.text(
                serde_json::to_string(&msg)
                    .unwrap_or(String::from("failed to serialize on server")),
            )
        }
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for AudioNodeSession {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        if let Ok(ws::Message::Close(reason)) = msg {
            ctx.close(reason.clone());
            ctx.stop();
        }
    }
}
