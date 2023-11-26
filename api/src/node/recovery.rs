use std::{thread, time::Duration};

use actix::{AsyncContext, Handler, Message};

use super::{
    health::AudioNodeHealth, node_server::AudioNode,
    processor_communication::AudioProcessorToNodeMessage,
};

const DEVICE_RECOVERY_ATTEMPT_INTERVAL: Duration = Duration::from_secs(5);

/// Used to try and recover connections with devices. Mainly intended to be used with bluetooth
/// devices.
///
/// On successful recovery playback resumes at the current queue head index and at the audio
/// progress that was last saved by the server (this should be the same as the last audio progress
/// that was sent to any client).
#[derive(Debug, Clone, Message)]
#[rtype(result = "()")]
pub struct TryRecoverDevice;

impl Handler<TryRecoverDevice> for AudioNode {
    type Result = ();

    #[allow(clippy::collapsible_else_if)]
    fn handle(&mut self, _msg: TryRecoverDevice, ctx: &mut Self::Context) -> Self::Result {
        match self.health {
            AudioNodeHealth::Good => {}
            _ => {
                let device_health_restored = if let Err(err) = self
                    .player
                    .try_recover_device(self.current_processor_info.audio_progress)
                {
                    log::error!(
                        "failed to recover device for node with source name {}\nERROR: {err}",
                        self.source_name
                    );
                    false
                } else {
                    true
                };

                if !device_health_restored {
                    thread::sleep(DEVICE_RECOVERY_ATTEMPT_INTERVAL);

                    if let Err(err) = ctx.address().try_send(TryRecoverDevice) {
                        log::error!("failed to resend 'try device revocer' message\nERROR: {err}");
                    };
                } else {
                    if let Err(err) = ctx
                        .address()
                        .try_send(AudioProcessorToNodeMessage::Health(AudioNodeHealth::Good))
                    {
                        log::error!(
                            "failed to inform node with source name {} of recovery\nERROR: {err}",
                            self.source_name
                        )
                    };
                }
            }
        };
    }
}
