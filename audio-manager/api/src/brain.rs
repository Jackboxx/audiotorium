use std::collections::HashMap;

use actix::{Actor, ActorContext, Addr, AsyncContext, Context, Handler, Message, Recipient};

use log::{error, info};
use serde::{Deserialize, Serialize};

use crate::{
    audio::{AudioPlayer, PlaybackInfo, ProcessorInfo},
    downloader::{
        self, AudioDownloader, DownloadAudio, DownloadAudioResponse, NotifyDownloadFinished,
    },
    session::{AudioBrainSessionPassThroughMessages, FilteredPassThroughtMessage},
    utils::create_source,
    ErrorResponse, AUDIO_DIR, AUDIO_SOURCES,
};

pub struct AudioBrain {
    downloader_addr: Addr<AudioDownloader>,
    sources: HashMap<String, AudioPlayer>,
    sessions: HashMap<usize, Recipient<FilteredPassThroughtMessage>>,
}

#[derive(Debug, Clone, Message)]
#[rtype(result = "Result<ConnectServerResponse, ()>")]
pub struct Connect {
    pub addr: Recipient<FilteredPassThroughtMessage>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConnectServerResponse {
    pub id: usize,
    pub sources: Vec<String>,
}

#[derive(Debug, Clone, Message)]
#[rtype(result = "()")]
pub struct Disconnect {
    pub id: usize,
}

impl AudioBrain {
    pub fn new(downloader_addr: Addr<AudioDownloader>) -> Self {
        Self {
            downloader_addr,
            sources: HashMap::default(),
            sessions: HashMap::default(),
        }
    }

    fn send_filtered_passthrought_msg(
        sessions: &[Recipient<FilteredPassThroughtMessage>],
        msg: String,
        source_name: &str,
    ) {
        for addr in sessions {
            addr.do_send(FilteredPassThroughtMessage {
                msg: msg.clone(),
                source_name: source_name.to_owned(),
            });
        }
    }
}

impl Actor for AudioBrain {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        // check this if weird shit happens just trying stuff here
        ctx.set_mailbox_capacity(64);
        info!("stared new 'AudioBrain', CONTEXT: {ctx:?}");

        for (human_readable_name, source_name) in AUDIO_SOURCES {
            let source = create_source(source_name, ctx.address());
            self.sources.insert(human_readable_name.to_owned(), source);
        }

        match self.downloader_addr.try_send(downloader::Connect {
            addr: ctx.address().into(),
        }) {
            Ok(_) => {}
            Err(err) => {
                error!("'AudioBrain' failed to connect to 'AudioDownloader', ERROR: {err}");
                ctx.stop();
            }
        };
    }
}

impl Handler<Connect> for AudioBrain {
    type Result = Result<ConnectServerResponse, ()>;
    fn handle(&mut self, msg: Connect, _ctx: &mut Self::Context) -> Self::Result {
        let Connect { addr } = msg;
        let id = self.sessions.keys().max().unwrap_or(&0) + 1;

        self.sessions.insert(id, addr);

        Ok(ConnectServerResponse {
            id,
            sources: self.sources.keys().map(|key| key.to_owned()).collect(),
        })
    }
}

impl Handler<Disconnect> for AudioBrain {
    type Result = ();
    fn handle(&mut self, msg: Disconnect, _ctx: &mut Self::Context) -> Self::Result {
        let Disconnect { id } = msg;
        self.sessions.remove(&id);
    }
}
