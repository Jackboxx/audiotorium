use actix::{
    Actor, ActorFutureExt, AsyncContext, Context, Handler, Message, Recipient, ResponseActFuture,
    WrapFuture,
};

use crate::{
    brain::brain_server::GetAudioNodeMessage,
    downloader::{self, actor::SerializableDownloadAudioRequest},
    error::AppError,
    node::node_server::SourceName,
    path::state_recovery_file_path,
    utils::log_msg_received,
};

use super::{AppStateRecoveryInfo, AudioStateInfo, DownloadStateInfo};

const STORE_INTERVAL: std::time::Duration = std::time::Duration::from_millis(3000);

#[derive(Debug, Default)]
pub struct RestoreStateActor {
    current_state: AppStateRecoveryInfo,
    has_changed: bool,
}

impl RestoreStateActor {
    pub async fn load_or_default() -> Self {
        let mut state: AppStateRecoveryInfo = match std::fs::read(state_recovery_file_path()) {
            Ok(bytes) => bincode::deserialize(&bytes).unwrap_or_default(),
            Err(_) => Default::default(),
        };

        for audio_state in state.audio_info.values_mut() {
            audio_state.restore_queue().await;
        }

        Self {
            current_state: state,
            ..Default::default()
        }
    }

    pub fn state(&self) -> AppStateRecoveryInfo {
        self.current_state.clone()
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

        ctx.notify(StoreState);
    }
}

#[derive(Debug, Message)]
#[rtype(result = "()")]
struct StoreState;

#[derive(Debug, Message)]
#[rtype(result = "()")]
pub struct RestoreDownloadQueue {
    pub get_node_addr_addr: Recipient<GetAudioNodeMessage>,
    pub download_addr: Recipient<downloader::actor::RestoreQueue>,
}

impl Handler<StoreState> for RestoreStateActor {
    type Result = ResponseActFuture<Self, ()>;

    fn handle(&mut self, msg: StoreState, _ctx: &mut Self::Context) -> Self::Result {
        log_msg_received(&self, &msg);

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

impl Handler<RestoreDownloadQueue> for RestoreStateActor {
    type Result = ResponseActFuture<Self, ()>;

    fn handle(&mut self, msg: RestoreDownloadQueue, _ctx: &mut Self::Context) -> Self::Result {
        log_msg_received(&self, &msg);

        let serialized_queue = dbg!(self.current_state.download_info.queue.clone());
        self.current_state.download_info.restored = true;
        Box::pin(
            async move {
                DownloadStateInfo::restore_queue(&serialized_queue, msg.get_node_addr_addr).await
            }
            .into_actor(self)
            .map(move |res, _, _| {
                msg.download_addr
                    .do_send(downloader::actor::RestoreQueue(dbg!(res)));
            }),
        )
    }
}

#[derive(Debug, Message)]
#[rtype(result = "()")]
pub struct DownloadQueueStateUpdateMessage(pub Vec<SerializableDownloadAudioRequest>);

impl Handler<DownloadQueueStateUpdateMessage> for RestoreStateActor {
    type Result = ();

    fn handle(
        &mut self,
        msg: DownloadQueueStateUpdateMessage,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        if !self.current_state.download_info.restored
            || self.current_state.download_info.queue.eq(&msg.0)
        {
            return;
        }

        log_msg_received(&self, &msg);
        self.current_state.download_info.queue = msg.0;
        self.has_changed = true;
    }
}

#[derive(Debug, Message)]
#[rtype(result = "()")]
pub struct AudioInfoStateUpdateMessage(pub (SourceName, AudioStateInfo));

impl Handler<AudioInfoStateUpdateMessage> for RestoreStateActor {
    type Result = ();

    fn handle(
        &mut self,
        msg: AudioInfoStateUpdateMessage,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        // log_msg_received(&self, &msg);

        let (source_name, info) = msg.0;

        self.current_state.audio_info.insert(source_name, info);
        self.has_changed = true;
    }
}
