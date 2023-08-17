use actix::{
    Actor, ActorContext, ActorFutureExt, Addr, AsyncContext, ContextFutureSpawner, Handler,
    Message, Running, StreamHandler, WrapFuture,
};
use actix_web_actors::ws;
use log::{error, info};
use serde::{Deserialize, Serialize};

use crate::{server::AudioBrain, ErrorResponse};

#[derive(Debug, Clone)]
pub struct AudioBrainSession {
    id: usize,
    server_addr: Addr<AudioBrain>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[allow(clippy::enum_variant_names)]
pub enum AudioBrainSessionUpdateMessage {
    SourceInformationUpdate,
    StartedDownloadingAudio,
    FinishedDownloadingAudio(FinishedDownloadingAudioServerResponse),
}

impl AudioBrainSession {
    pub fn new(server_addr: Addr<AudioBrain>) -> Self {
        Self {
            id: usize::MAX,
            server_addr,
        }
    }
}

impl Actor for AudioBrainSession {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        // info!("stared new 'AudioBrainSession'");

        // let addr = ctx.address();
        // self.server_addr
        //     .send(Connect {
        //         addr: addr.recipient(),
        //     })
        //     .into_actor(self)
        //     .then(|res, act, ctx| {
        //         match res {
        //             Ok(res) => match res {
        //                 Ok(params) => {
        //                     info!("'AudioBrainSession' connected");
        //                     act.id = params.id;

        //                     ctx.text(
        //                         serde_json::to_string(
        //                             &AudioBrainMessageResponse::SessionConnectedResponse(params),
        //                         )
        //                         .unwrap_or("[]".to_owned()),
        //                     );
        //                 }
        //                 Err(err) => {
        //                     error!(
        //                         "'AudioBrainSession' failed to connect to 'AudioBrain', ERROR: {err:?}"
        //                     );
        //                     ctx.stop();
        //                 }
        //             },

        //             Err(err) => {
        //                 error!("'AudioBrainSession' failed to connect to 'AudioBrain', ERROR: {err}");
        //                 ctx.stop();
        //             }
        //         }

        //         actix::fut::ready(())
        //     })
        //     .wait(ctx);
    }

    fn stopping(&mut self, _: &mut Self::Context) -> Running {
        info!("'AudioBrainSession' stopping, ID: {}", self.id);

        self.server_addr.do_send(Disconnect { id: self.id });
        Running::Stop
    }
}
