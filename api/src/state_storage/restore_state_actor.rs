use actix::{
    Actor, ActorFutureExt, AsyncContext, Context, Handler, Message, ResponseActFuture, WrapFuture,
};

use crate::{
    audio_playback::audio_player::AudioInfo, downloader::actor::SerializableDownloadAudioRequest,
    error::AppError, node::node_server::SourceName, path::state_recovery_file_path,
};

use super::AppStateRecoveryInfo;

const STORE_INTERVAL: std::time::Duration = std::time::Duration::from_millis(3000);

#[derive(Debug, Default)]
pub struct RestoreStateActor {
    current_state: AppStateRecoveryInfo,
    has_changed: bool,
}

impl RestoreStateActor {
    pub fn load_or_default() -> Self {
        let state: AppStateRecoveryInfo = match std::fs::read(state_recovery_file_path()) {
            Ok(bytes) => bincode::deserialize(&bytes).unwrap_or_default(),
            Err(_) => Default::default(),
        };

        Self {
            current_state: state,
            ..Default::default()
        }
    }

    fn store_state(&self) -> Result<(), AppError> {
        let bin = bincode::serialize(&self.current_state).unwrap();
        std::fs::write(state_recovery_file_path(), bin).unwrap();

        Ok(())
    }
}

impl Actor for RestoreStateActor {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        log::info!("stared new 'RestoreStateActor', CONTEXT: {ctx:?}");
    }
}

#[derive(Debug, Message)]
#[rtype(result = "()")]
struct StoreState;

impl Handler<StoreState> for RestoreStateActor {
    type Result = ResponseActFuture<Self, ()>;

    fn handle(&mut self, _msg: StoreState, _ctx: &mut Self::Context) -> Self::Result {
        if self.has_changed {
            let _ = self.store_state();
            self.has_changed = false;
        }

        Box::pin(
            async {
                actix_rt::time::sleep(STORE_INTERVAL).await;
            }
            .into_actor(self)
            .map(|_, _, ctx| {
                ctx.notify(StoreState);
            }),
        )
    }
}

#[derive(Debug, Message)]
#[rtype(result = "()")]
pub struct DownloadQueueStateUpdateMessage(Vec<SerializableDownloadAudioRequest>);

impl Handler<DownloadQueueStateUpdateMessage> for RestoreStateActor {
    type Result = ();

    fn handle(
        &mut self,
        msg: DownloadQueueStateUpdateMessage,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        self.current_state.download_info.queue = msg.0;
        self.has_changed = true;
    }
}

#[derive(Debug, Message)]
#[rtype(result = "()")]
pub struct AudioInfoStateUpdateMessage((SourceName, AudioInfo));

impl Handler<AudioInfoStateUpdateMessage> for RestoreStateActor {
    type Result = ();

    fn handle(
        &mut self,
        msg: AudioInfoStateUpdateMessage,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        let (source_name, info) = msg.0;

        self.current_state.audio_info.insert(source_name, info);
        self.has_changed = true;
    }
}
