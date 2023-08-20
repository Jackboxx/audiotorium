use std::collections::HashMap;

use actix::{Actor, Addr, AsyncContext, Context, Handler, Message, MessageResponse};

use serde::Serialize;

use crate::{
    brain_session::AudioBrainSession,
    downloader::AudioDownloader,
    node::{AudioNode, AudioNodeHealth, AudioNodeInfo},
    utils::create_player,
    AUDIO_SOURCES,
};

pub struct AudioBrain {
    downloader_addr: Addr<AudioDownloader>,
    nodes: HashMap<String, (Addr<AudioNode>, AudioNodeInfo)>,
    sessions: HashMap<usize, Addr<AudioBrainSession>>,
}

#[derive(Debug, Clone, Message)]
#[rtype(result = "BrainConnectResponse")]
pub struct BrainConnect {
    pub addr: Addr<AudioBrainSession>,
}

#[derive(Debug, Clone, Serialize, MessageResponse)]
pub struct BrainConnectResponse {
    pub id: usize,
    pub sources: Vec<AudioNodeInfo>,
}

#[derive(Debug, Clone, Message)]
#[rtype(result = "()")]
pub struct BrainDisconnect {
    pub id: usize,
}

impl AudioBrain {
    pub fn new(downloader_addr: Addr<AudioDownloader>) -> Self {
        Self {
            downloader_addr,
            nodes: HashMap::default(),
            sessions: HashMap::default(),
        }
    }

    fn multicast<M>(&self, msg: M)
    where
        M: Message + Send + Clone + 'static,
        M::Result: Send,
        AudioBrainSession: Handler<M>,
    {
        for (_, addr) in &self.sessions {
            addr.do_send(msg.clone());
        }
    }
}

impl Actor for AudioBrain {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        // check this if weird shit happens just trying stuff here
        ctx.set_mailbox_capacity(64);
        log::info!("stared new 'AudioBrain', CONTEXT: {ctx:?}");

        for (human_readable_name, source_name) in AUDIO_SOURCES {
            let player = create_player(source_name);
            let node = AudioNode::new(player, ctx.address(), self.downloader_addr.clone());
            let node_addr = node.start();

            self.nodes.insert(
                source_name.to_owned(),
                (
                    node_addr,
                    AudioNodeInfo {
                        source_name: source_name.to_owned(),
                        human_readable_name: human_readable_name.to_owned(),
                        health: AudioNodeHealth::Good,
                    },
                ),
            );
        }
    }
}

impl Handler<BrainConnect> for AudioBrain {
    type Result = BrainConnectResponse;

    fn handle(&mut self, msg: BrainConnect, _ctx: &mut Self::Context) -> Self::Result {
        let BrainConnect { addr } = msg;
        let id = self.sessions.keys().max().unwrap_or(&0) + 1;

        self.sessions.insert(id, addr);

        BrainConnectResponse {
            id,
            sources: self
                .nodes
                .values()
                .map(|(_, info)| info.to_owned())
                .collect(),
        }
    }
}

impl Handler<BrainDisconnect> for AudioBrain {
    type Result = ();
    fn handle(&mut self, msg: BrainDisconnect, _ctx: &mut Self::Context) -> Self::Result {
        let BrainDisconnect { id } = msg;
        self.sessions.remove(&id);
    }
}
