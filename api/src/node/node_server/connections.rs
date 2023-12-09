use actix::{Addr, Handler, Message, MessageResponse};

use crate::{
    audio_playback::audio_player::PlaybackInfo,
    node::node_session::{AudioNodeSession, NodeSessionWsResponse},
    streams::node_streams::{AudioNodeInfoStreamType, AudioStateInfo, RunningDownloadInfo},
    utils::log_msg_received,
};

use super::{extract_queue_metadata, AudioNode};

#[derive(Debug, Clone, Message)]
#[rtype(result = "NodeConnectResponse")]
pub struct NodeConnectMessage {
    pub addr: Addr<AudioNodeSession>,
    pub wanted_info: Vec<AudioNodeInfoStreamType>,
}

#[derive(Debug, Clone, Message)]
#[rtype(result = "()")]
pub struct NodeDisconnectMessage {
    pub id: usize,
}

#[derive(Debug, Clone, MessageResponse)]
pub struct NodeConnectResponse {
    pub id: usize,
    pub connection_response: NodeSessionWsResponse,
}

impl Handler<NodeConnectMessage> for AudioNode {
    type Result = NodeConnectResponse;

    fn handle(&mut self, msg: NodeConnectMessage, _ctx: &mut Self::Context) -> Self::Result {
        log_msg_received(&self, &msg);

        let id = self.sessions.keys().max().unwrap_or(&0) + 1;
        self.sessions.insert(id, msg.addr);

        let connection_response = NodeSessionWsResponse::SessionConnectedResponse {
            queue: msg
                .wanted_info
                .contains(&AudioNodeInfoStreamType::Queue)
                .then_some(extract_queue_metadata(self.player.queue())),
            health: msg
                .wanted_info
                .contains(&AudioNodeInfoStreamType::Health)
                .then_some(self.health.clone()),
            downloads: msg
                .wanted_info
                .contains(&AudioNodeInfoStreamType::Download)
                .then_some(RunningDownloadInfo {
                    active: self.active_downloads.clone().into_iter().collect(),
                    failed: self.failed_downloads.clone().into_iter().collect(),
                }),
            audio_state_info: msg
                .wanted_info
                .contains(&AudioNodeInfoStreamType::AudioStateInfo)
                .then_some(AudioStateInfo {
                    playback_info: PlaybackInfo {
                        current_head_index: self.player.queue_head(),
                    },
                    processor_info: self.current_processor_info.clone(),
                }),
        };

        NodeConnectResponse {
            id,
            connection_response,
        }
    }
}

impl Handler<NodeDisconnectMessage> for AudioNode {
    type Result = ();

    fn handle(&mut self, msg: NodeDisconnectMessage, _ctx: &mut Self::Context) -> Self::Result {
        log_msg_received(&self, &msg);

        self.sessions.remove(&msg.id);
    }
}