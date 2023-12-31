use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::Arc,
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
    error::AppError,
    state_storage::restore_state_actor::RestoreStateActor,
};

use super::{health::AudioNodeHealth, node_session::AudioNodeSession};

pub mod async_actor;
pub mod connections;
pub mod download_notifications;
pub mod sync_actor;

pub type SourceName = Arc<str>;

pub struct AudioNode {
    pub(super) source_name: SourceName,
    pub(super) current_processor_info: ProcessorInfo,
    pub(super) player: AudioPlayer<PathBuf>,
    pub(super) downloader_addr: Addr<AudioDownloader>,
    pub(super) restore_state_addr: Addr<RestoreStateActor>,
    pub(super) active_downloads: HashSet<DownloadInfo>,
    pub(super) failed_downloads: HashMap<DownloadInfo, AppError>,
    pub(super) server_addr: Addr<AudioBrain>,
    pub(super) sessions: HashMap<usize, Addr<AudioNodeSession>>,
    pub(super) health: AudioNodeHealth,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../app/src/api-types/")]
pub struct AudioNodeInfo {
    pub source_name: SourceName,
    pub human_readable_name: String,
    pub health: AudioNodeHealth,
}

#[derive(Debug, PartialEq, Eq)]
pub enum UrlKindByProvider {
    Youtube,
}

#[derive(Debug)]
pub enum AudioUrl {
    Youtube(Arc<str>),
}

impl Actor for AudioNode {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        log::info!("stared new 'AudioNode', CONTEXT: {ctx:?}");

        self.player.set_addr(Some(ctx.address()))
    }
}

impl Clone for AudioUrl {
    fn clone(&self) -> Self {
        match self {
            Self::Youtube(url) => Self::Youtube(Arc::clone(url)),
        }
    }
}

impl AudioUrl {
    fn inner(&self) -> Arc<str> {
        match self {
            Self::Youtube(url) => Arc::clone(url),
        }
    }

    fn kind(&self) -> UrlKindByProvider {
        match self {
            Self::Youtube(_) => UrlKindByProvider::Youtube,
        }
    }
}

impl AudioNode {
    pub fn new(
        source_name: SourceName,
        player: AudioPlayer<PathBuf>,
        server_addr: Addr<AudioBrain>,
        downloader_addr: Addr<AudioDownloader>,
        restore_state_addr: Addr<RestoreStateActor>,
    ) -> Self {
        Self {
            source_name,
            current_processor_info: ProcessorInfo::new(1.0),
            player,
            downloader_addr,
            restore_state_addr,
            server_addr,
            active_downloads: HashSet::default(),
            failed_downloads: HashMap::default(),
            sessions: HashMap::default(),
            health: AudioNodeHealth::Good,
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

// TODO make more detailed an properly handle invalid URLs

/// remove trailing parameters from URL
pub fn clean_url(url: &str) -> &str {
    url.split_once('&').map(|(str, _)| str).unwrap_or(url)
}

pub fn extract_queue_metadata<ADL: AudioDataLocator>(
    queue: &[AudioPlayerQueueItem<ADL>],
) -> SerializableQueue {
    queue.iter().map(|item| item.metadata.clone()).collect()
}
