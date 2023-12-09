use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use actix::{Actor, Addr, AsyncContext, Context, Handler, Message};
use serde::Serialize;
use ts_rs::TS;

use crate::{
    audio_playback::{
        audio_item::{AudioDataLocator, AudioPlayerQueueItem},
        audio_player::{AudioPlayer, ProcessorInfo, SerializableQueue},
    },
    brain::brain_server::AudioBrain,
    downloader::{actor::AudioDownloader, info::DownloadInfo},
    ErrorResponse,
};

use super::{health::AudioNodeHealth, node_session::AudioNodeSession};

pub mod async_actor;
pub mod connections;
pub mod download_notifications;
pub mod sync_actor;

pub type SourceName = String;

pub struct AudioNode {
    pub(super) source_name: SourceName,
    pub(super) current_processor_info: ProcessorInfo,
    pub(super) player: AudioPlayer<PathBuf>,
    pub(super) downloader_addr: Addr<AudioDownloader>,
    pub(super) active_downloads: HashSet<DownloadInfo>,
    pub(super) failed_downloads: HashMap<DownloadInfo, ErrorResponse>,
    pub(super) server_addr: Addr<AudioBrain>,
    pub(super) sessions: HashMap<usize, Addr<AudioNodeSession>>,
    pub(super) health: AudioNodeHealth,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../app/src/api-types/")]
pub struct AudioNodeInfo {
    pub source_name: String,
    pub human_readable_name: String,
    pub health: AudioNodeHealth,
}

impl Actor for AudioNode {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        log::info!("stared new 'AudioNode', CONTEXT: {ctx:?}");

        self.player.set_addr(Some(ctx.address()))
    }
}

impl AudioNode {
    pub fn new(
        source_name: String,
        player: AudioPlayer<PathBuf>,
        server_addr: Addr<AudioBrain>,
        downloader_addr: Addr<AudioDownloader>,
    ) -> Self {
        Self {
            source_name,
            player,
            downloader_addr,
            server_addr,
            active_downloads: HashSet::default(),
            failed_downloads: HashMap::default(),
            sessions: HashMap::default(),
            health: AudioNodeHealth::Good,
            current_processor_info: ProcessorInfo::new(1.0),
        }
    }

    pub(super) fn multicast<M>(&self, msg: M)
    where
        M: Message + Send + Clone + 'static,
        M::Result: Send,
        AudioNodeSession: Handler<M>,
    {
        for addr in self.sessions.values() {
            addr.do_send(msg.clone());
        }
    }

    pub(super) fn multicast_result<MOk, MErr>(&self, msg: Result<MOk, MErr>)
    where
        MOk: Message + Send + Clone + 'static,
        MOk::Result: Send,
        AudioNodeSession: Handler<MOk>,
        MErr: Message + Send + Clone + 'static,
        MErr::Result: Send,
        AudioNodeSession: Handler<MErr>,
    {
        match msg {
            Ok(msg) => {
                for addr in self.sessions.values() {
                    addr.do_send(msg.clone());
                }
            }
            Err(msg) => {
                for addr in self.sessions.values() {
                    addr.do_send(msg.clone());
                }
            }
        }
    }
}

pub fn extract_queue_metadata<ADL: AudioDataLocator>(
    queue: &[AudioPlayerQueueItem<ADL>],
) -> SerializableQueue {
    queue.iter().map(|item| item.metadata.clone()).collect()
}
