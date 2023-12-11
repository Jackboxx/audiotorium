use crate::{
    audio_playback::audio_player::{PlaybackState, SerializableQueue},
    commands::node_commands::{AudioNodeCommand, MoveQueueItemParams, RemoveQueueItemParams},
    error::{AppError, AppErrorKind, IntoAppError},
    node::node_server::async_actor::AsyncAddQueueItem,
    streams::node_streams::AudioNodeInfoStreamMessage,
    utils::log_msg_received,
};

use actix::{AsyncContext, Handler};

use super::{extract_queue_metadata, AudioNode};

impl Handler<AudioNodeCommand> for AudioNode {
    type Result = Result<(), AppError>;

    fn handle(&mut self, msg: AudioNodeCommand, ctx: &mut Self::Context) -> Self::Result {
        log_msg_received(&self, &msg);

        match &msg {
            AudioNodeCommand::AddQueueItem(params) => {
                log::info!("'AddQueueItem' handler received a message, MESSAGE: {msg:?}");

                ctx.notify(AsyncAddQueueItem(params.clone()));
                Ok(())
            }
            AudioNodeCommand::RemoveQueueItem(params) => {
                log::info!("'RemoveQueueItem' handler received a message, MESSAGE: {msg:?}");

                let msg = AudioNodeInfoStreamMessage::Queue(handle_remove_queue_item(
                    self,
                    params.clone(),
                )?);
                self.multicast(msg);

                Ok(())
            }
            AudioNodeCommand::MoveQueueItem(params) => {
                log::info!("'MoveQueueItem' handler received a message, MESSAGE: {msg:?}");

                let msg =
                    AudioNodeInfoStreamMessage::Queue(handle_move_queue_item(self, params.clone()));

                self.multicast(msg);

                Ok(())
            }
            AudioNodeCommand::SetAudioVolume(params) => {
                log::info!("'SetAudioVolume' handler received a message, MESSAGE: {msg:?}");

                self.player.set_volume(params.volume);
                Ok(())
            }
            AudioNodeCommand::SetAudioProgress(params) => {
                log::info!("'SetAudioProgress' handler received a message, MESSAGE: {msg:?}");

                self.player.set_stream_progress(params.progress);
                Ok(())
            }
            AudioNodeCommand::PauseQueue => {
                log::info!("'PauseQueue' handler received a message, MESSAGE: {msg:?}");

                self.player.set_stream_playback_state(PlaybackState::Paused);
                Ok(())
            }
            AudioNodeCommand::UnPauseQueue => {
                log::info!("'UnPauseQueue' handler received a message, MESSAGE: {msg:?}");

                self.player
                    .set_stream_playback_state(PlaybackState::Playing);
                Ok(())
            }
            AudioNodeCommand::PlayNext => {
                log::info!("'PlayNext' handler received a message, MESSAGE: {msg:?}");

                self.player.play_next().into_app_err(
                    "failed to play next audio",
                    AppErrorKind::Queue,
                    &[&format!("NODE_NAME: {name}", name = self.source_name)],
                )?;
                Ok(())
            }
            AudioNodeCommand::PlayPrevious => {
                log::info!("'PlayPrevious' handler received a message, MESSAGE: {msg:?}");

                self.player.play_prev().into_app_err(
                    "failed to play previous audio",
                    AppErrorKind::Queue,
                    &[&format!("NODE_NAME: {name}", name = self.source_name)],
                )?;
                Ok(())
            }
            AudioNodeCommand::PlaySelected(params) => {
                log::info!("'PlaySelected' handler received a message, MESSAGE: {msg:?}");

                self.player
                    .play_selected(params.index, false)
                    .into_app_err(
                        "failed to play selected audio",
                        AppErrorKind::Queue,
                        &[
                            &format!("NODE_NAME: {name}", name = self.source_name),
                            &format!("INDEX: {index}", index = params.index),
                        ],
                    )?;
                Ok(())
            }
            AudioNodeCommand::LoopQueue(params) => {
                log::info!("'LoopQueue' handler received a message, MESSAGE: {msg:?}");

                self.player.set_loop(params.bounds.clone());
                Ok(())
            }
        }
    }
}

fn handle_remove_queue_item(
    node: &mut AudioNode,
    params: RemoveQueueItemParams,
) -> Result<SerializableQueue, AppError> {
    let RemoveQueueItemParams { index } = params.clone();

    if let Err(err) = node.player.remove_from_queue(index) {
        log::error!("failed to play correct audio after removing element from queue, MESSAGE: {params:?}, ERROR: {err}");
        return Err(err.into_app_err(
            "failed to play correct audio after removing item",
            AppErrorKind::Queue,
            &[&format!("NODE_NAME: {name}", name = node.source_name)],
        ));
    }

    Ok(extract_queue_metadata(node.player.queue()))
}

fn handle_move_queue_item(node: &mut AudioNode, params: MoveQueueItemParams) -> SerializableQueue {
    let MoveQueueItemParams { old_pos, new_pos } = params;
    node.player.move_queue_item(old_pos, new_pos);

    extract_queue_metadata(node.player.queue())
}
