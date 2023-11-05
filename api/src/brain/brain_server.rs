use std::collections::HashMap;

use actix::{Actor, Addr, AsyncContext, Context, Handler, Message, MessageResponse};

use crate::{
    audio::audio_player::AudioPlayer,
    downloader::AudioDownloader,
    node::node_server::{AudioNode, AudioNodeHealth, AudioNodeInfo, SourceName},
    streams::brain_streams::{AudioBrainInfoStreamMessage, AudioBrainInfoStreamType},
    utils::{get_audio_sources, log_msg_received},
};

use super::brain_session::{AudioBrainSession, BrainSessionWsResponse};

pub struct AudioBrain {
    downloader_addr: Addr<AudioDownloader>,
    nodes: HashMap<SourceName, (Addr<AudioNode>, AudioNodeInfo)>,
    sessions: HashMap<usize, Addr<AudioBrainSession>>,
}

#[derive(Debug, Clone, Message)]
#[rtype(result = "Option<Addr<AudioNode>>")]
pub struct GetAudioNodeMessage {
    pub source_name: String,
}

#[derive(Debug, Clone, Message)]
#[rtype(result = "()")]
pub enum AudioNodeToBrainMessage {
    NodeHealthUpdate((String, AudioNodeHealth)),
}

#[derive(Debug, Clone, Message)]
#[rtype(result = "BrainConnectResponse")]
pub struct BrainConnectMessage {
    pub addr: Addr<AudioBrainSession>,
    pub wanted_info: Vec<AudioBrainInfoStreamType>,
}

#[derive(Debug, Clone, MessageResponse)]
pub struct BrainConnectResponse {
    pub id: usize,
    pub connection_response: BrainSessionWsResponse,
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
        for addr in self.sessions.values() {
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

        for (source_name, info) in get_audio_sources().into_iter() {
            if let Ok(player) = AudioPlayer::try_new(source_name.to_owned(), None) {
                let node = AudioNode::new(
                    source_name.to_owned(),
                    player,
                    ctx.address(),
                    self.downloader_addr.clone(),
                );
                let node_addr = node.start();

                self.nodes.insert(
                    source_name.to_owned(),
                    (
                        node_addr,
                        AudioNodeInfo {
                            source_name: source_name.to_owned(),
                            human_readable_name: info.human_readable_name.clone(),
                            health: AudioNodeHealth::Good,
                        },
                    ),
                );
            }
        }
    }
}

impl Handler<BrainConnectMessage> for AudioBrain {
    type Result = BrainConnectResponse;

    fn handle(&mut self, msg: BrainConnectMessage, _ctx: &mut Self::Context) -> Self::Result {
        log_msg_received(&self, &msg);

        let BrainConnectMessage { addr, wanted_info } = msg;
        let id = self.sessions.keys().max().unwrap_or(&0) + 1;

        self.sessions.insert(id, addr);

        let connection_response = if wanted_info.contains(&AudioBrainInfoStreamType::NodeInfo) {
            BrainSessionWsResponse::SessionConnectedResponse {
                node_info: Some(
                    self.nodes
                        .values()
                        .map(|(_, info)| info.to_owned())
                        .collect(),
                ),
            }
        } else {
            BrainSessionWsResponse::SessionConnectedResponse { node_info: None }
        };

        BrainConnectResponse {
            id,
            connection_response,
        }
    }
}

impl Handler<BrainDisconnect> for AudioBrain {
    type Result = ();
    fn handle(&mut self, msg: BrainDisconnect, _ctx: &mut Self::Context) -> Self::Result {
        log_msg_received(&self, &msg);

        let BrainDisconnect { id } = msg;
        self.sessions.remove(&id);
    }
}

impl Handler<AudioNodeToBrainMessage> for AudioBrain {
    type Result = ();

    fn handle(&mut self, msg: AudioNodeToBrainMessage, _ctx: &mut Self::Context) -> Self::Result {
        log_msg_received(&self, &msg);

        match &msg {
            AudioNodeToBrainMessage::NodeHealthUpdate(params) => {
                let (source_name, health) = params;

                if let Some((_, node_info)) = self.nodes.get_mut(source_name) {
                    node_info.health = health.clone();

                    let msg = AudioBrainInfoStreamMessage::NodeInfo(
                        self.nodes
                            .values()
                            .map(|(_, info)| info.to_owned())
                            .collect(),
                    );

                    self.multicast(msg)
                }
            }
        }
    }
}

impl Handler<GetAudioNodeMessage> for AudioBrain {
    type Result = Option<Addr<AudioNode>>;

    fn handle(&mut self, msg: GetAudioNodeMessage, _ctx: &mut Self::Context) -> Self::Result {
        log_msg_received(&self, &msg);

        self.nodes
            .get(&msg.source_name)
            .and_then(|v| match v.1.health {
                AudioNodeHealth::Poor(_) => None,
                _ => Some(v.0.clone()),
            })
    }
}