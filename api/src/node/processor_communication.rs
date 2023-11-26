use actix::{AsyncContext, Handler, Message};

use crate::{
    audio_playback::audio_player::{PlaybackInfo, ProcessorInfo},
    brain::brain_server::AudioNodeToBrainMessage,
    streams::node_streams::{AudioNodeInfoStreamMessage, AudioStateInfo},
    utils::log_msg_received,
};

use super::{health::AudioNodeHealth, node_server::AudioNode, recovery::TryRecoverDevice};

/// Used to communicate between the audio player and the audio node.
#[derive(Debug, Clone, Message, PartialEq)]
#[rtype(result = "()")]
pub enum AudioProcessorToNodeMessage {
    AudioStateInfo(ProcessorInfo),
    Health(AudioNodeHealth),
}

impl Handler<AudioProcessorToNodeMessage> for AudioNode {
    type Result = ();

    fn handle(
        &mut self,
        msg: AudioProcessorToNodeMessage,
        ctx: &mut Self::Context,
    ) -> Self::Result {
        match msg {
            AudioProcessorToNodeMessage::AudioStateInfo(_) => {}
            _ => {
                log_msg_received(&self, &msg);
            }
        }

        match msg {
            AudioProcessorToNodeMessage::Health(health) => {
                self.health = health.clone();

                self.server_addr
                    .do_send(AudioNodeToBrainMessage::NodeHealthUpdate((
                        self.source_name.to_owned(),
                        health.clone(),
                    )));

                self.multicast(AudioNodeInfoStreamMessage::Health(health));

                match self.health {
                    AudioNodeHealth::Good => {}
                    _ => {
                        if let Err(err) = ctx.address().try_send(TryRecoverDevice) {
                            log::error!(
                                "failed to send initial 'try device revocer' message\nERROR: {err}"
                            );
                        }
                    }
                };
            }
            AudioProcessorToNodeMessage::AudioStateInfo(processor_info) => {
                self.current_processor_info = processor_info.clone();

                let msg = AudioNodeInfoStreamMessage::AudioStateInfo(AudioStateInfo {
                    playback_info: PlaybackInfo {
                        current_head_index: self.player.queue_head(),
                    },
                    processor_info,
                });

                self.multicast(msg);
            }
        }
    }
}
