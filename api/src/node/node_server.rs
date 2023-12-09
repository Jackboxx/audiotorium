use crate::{
    audio_hosts::youtube::{
        playlist::get_playlist_video_urls, youtube_content_type, YoutubeContentType,
    },
    audio_playback::{
        audio_item::{AudioDataLocator, AudioMetaData, AudioPlayerQueueItem},
        audio_player::{
            AudioPlayer, PlaybackInfo, PlaybackState, ProcessorInfo, SerializableQueue,
        },
    },
    brain::brain_server::AudioBrain,
    commands::node_commands::{
        AddQueueItemParams, AudioNodeCommand, DownloadIdentifierParam, MoveQueueItemParams,
        RemoveQueueItemParams,
    },
    db_pool,
    downloader::{
        actor::{AudioDownloader, DownloadAudioRequest, NotifyDownloadUpdate},
        download_identifier::{
            DownloadRequiredInformation, Identifier, YoutubePlaylistDownloadInfo,
            YoutubePlaylistUrl, YoutubeVideoUrl,
        },
        info::DownloadInfo,
    },
    streams::node_streams::{
        AudioNodeInfoStreamMessage, AudioNodeInfoStreamType, AudioStateInfo, RunningDownloadInfo,
    },
    utils::log_msg_received,
    yt_api_key, ErrorResponse, IntoErrResp,
};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::Arc,
};

use actix::{
    Actor, ActorFutureExt, Addr, AsyncContext, Context, Handler, Message, MessageResponse,
    Recipient, ResponseActFuture, WrapFuture,
};
use anyhow::anyhow;
use serde::Serialize;
use ts_rs::TS;

use super::{
    health::AudioNodeHealth,
    node_session::{AudioNodeSession, NodeSessionWsResponse},
};

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

impl Actor for AudioNode {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        log::info!("stared new 'AudioNode', CONTEXT: {ctx:?}");

        self.player.set_addr(Some(ctx.address()))
    }
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

impl Handler<NotifyDownloadUpdate> for AudioNode {
    type Result = ();

    fn handle(&mut self, msg: NotifyDownloadUpdate, _ctx: &mut Self::Context) -> Self::Result {
        match msg {
            NotifyDownloadUpdate::Queued(info) => {
                self.active_downloads.insert(info);

                let msg = AudioNodeInfoStreamMessage::Download(RunningDownloadInfo {
                    active: self.active_downloads.clone().into_iter().collect(),
                    failed: self.failed_downloads.clone().into_iter().collect(),
                });

                self.multicast(msg);
            }
            NotifyDownloadUpdate::FailedToQueue((info, err_resp)) => {
                self.failed_downloads.insert(info, err_resp);

                let msg = AudioNodeInfoStreamMessage::Download(RunningDownloadInfo {
                    active: self.active_downloads.clone().into_iter().collect(),
                    failed: self.failed_downloads.clone().into_iter().collect(),
                });

                self.multicast(msg);
            }
            NotifyDownloadUpdate::SingleFinished(Ok((info, metadata, path))) => {
                self.active_downloads.remove(&info);
                self.failed_downloads.remove(&info);

                let item = AudioPlayerQueueItem {
                    metadata,
                    locator: path,
                };

                if let Err(err) = self.player.push_to_queue(item) {
                    log::error!("failed to auto play first song, ERROR: {err}");
                    return;
                };

                let download_fin_msg = AudioNodeInfoStreamMessage::Download(RunningDownloadInfo {
                    active: self.active_downloads.clone().into_iter().collect(),
                    failed: self.failed_downloads.clone().into_iter().collect(),
                });
                self.multicast(download_fin_msg);

                let updated_queue_msg =
                    AudioNodeInfoStreamMessage::Queue(extract_queue_metadata(self.player.queue()));
                self.multicast(updated_queue_msg);
            }
            NotifyDownloadUpdate::SingleFinished(Err((info, err_resp))) => {
                self.active_downloads.remove(&info);
                self.failed_downloads.insert(info, err_resp);

                let msg = AudioNodeInfoStreamMessage::Download(RunningDownloadInfo {
                    active: self.active_downloads.clone().into_iter().collect(),
                    failed: self.failed_downloads.clone().into_iter().collect(),
                });

                self.multicast(msg);
            }
            NotifyDownloadUpdate::BatchUpdated { batch } => match batch {
                DownloadInfo::YoutubePlaylist { ref video_urls, .. } => {
                    if video_urls.is_empty() {
                        self.active_downloads.remove(&batch);
                    } else {
                        self.active_downloads.replace(batch);
                    };

                    let msg = AudioNodeInfoStreamMessage::Download(RunningDownloadInfo {
                        active: self.active_downloads.clone().into_iter().collect(),
                        failed: self.failed_downloads.clone().into_iter().collect(),
                    });

                    self.multicast(msg);
                }
                _ => {
                    log::warn!("received a batch updated that wasn't a valid batch, valid batches are [youtube-playlist]");
                }
            },
        }
    }
}

impl Handler<AudioNodeCommand> for AudioNode {
    type Result = Result<(), ErrorResponse>;

    fn handle(&mut self, msg: AudioNodeCommand, ctx: &mut Self::Context) -> Self::Result {
        log_msg_received(&self, &msg);

        match &msg {
            AudioNodeCommand::AddQueueItem(params) => {
                log::info!("'AddQueueItem' handler received a message, MESSAGE: {msg:?}");

                ctx.notify(AsyncAudioNodeCommand::AddQueueItem(params.clone()));
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

                self.player.play_next().into_err_resp("")?;
                Ok(())
            }
            AudioNodeCommand::PlayPrevious => {
                log::info!("'PlayPrevious' handler received a message, MESSAGE: {msg:?}");

                self.player.play_prev().into_err_resp("")?;
                Ok(())
            }
            AudioNodeCommand::PlaySelected(params) => {
                log::info!("'PlaySelected' handler received a message, MESSAGE: {msg:?}");

                self.player
                    .play_selected(params.index, false)
                    .into_err_resp("")?;
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

#[derive(Debug, Clone, Message)]
#[rtype(result = "()")]
enum AsyncAudioNodeCommand {
    AddQueueItem(AddQueueItemParams),
}

impl DownloadIdentifierParam {
    async fn to_internal_identifier(self) -> anyhow::Result<DownloadRequiredInformation> {
        let url = match self {
            Self::Youtube { url } => url,
        };

        let content_type = youtube_content_type(url.as_str());

        match content_type {
            YoutubeContentType::Video => Ok(DownloadRequiredInformation::YoutubeVideo {
                url: YoutubeVideoUrl(url.into()),
            }),
            YoutubeContentType::Playlist => {
                let urls = match get_playlist_video_urls(&url, yt_api_key()).await {
                    Ok(urls) => urls,
                    Err(err) => {
                        log::error!("failed to get playlist information for youtube playlist, URL: {url}, ERROR: {err}");
                        return Err(anyhow!(
                            "failed to get playlist information for youtube playlist, URL: {url}"
                        ));
                    }
                };

                Ok(DownloadRequiredInformation::YoutubePlaylist(
                    YoutubePlaylistDownloadInfo {
                        playlist_url: YoutubePlaylistUrl(url.into()),
                        video_urls: urls.into_iter().map(Into::into).collect(),
                    },
                ))
            }
            YoutubeContentType::Invalid => Err(anyhow!("invalid youtube video url, URL: {url}")),
        }
    }
}

enum MetadataExists {
    Found {
        metadata: AudioMetaData,
        path: PathBuf,
    },
    NotFound {
        url: Arc<str>,
    },
}

impl Handler<AsyncAudioNodeCommand> for AudioNode {
    type Result = ResponseActFuture<Self, ()>;

    fn handle(&mut self, msg: AsyncAudioNodeCommand, _ctx: &mut Self::Context) -> Self::Result {
        enum MetadataQueryResult {
            Single(MetadataExists),
            Many(Vec<Option<AudioMetaData>>),
        }

        match msg {
            AsyncAudioNodeCommand::AddQueueItem(params) => {
                Box::pin(
                    async move {
                        let identifier = params.identifier.to_internal_identifier().await.unwrap();

                        // TODO: differentiate between video and playlist
                        //       - video:       use existing
                        //       - playlist:
                        //          1. query if playlist exists
                        //          2. create playlist table if not
                        //          3. create links to metadata for existing metadata
                        //          4. make request for remaining metadata

                        let query_res: Result<MetadataQueryResult, ErrorResponse> = match identifier
                        {
                            DownloadRequiredInformation::YoutubeVideo { url } => {
                                let uid = url.uid();
                                get_audio_metadata_from_db(&uid).await.map(|res| {
                                    MetadataQueryResult::Single(
                                        res.map(|md| MetadataExists::Found {
                                            metadata: md,
                                            path: url.to_path_with_ext(),
                                        })
                                        .unwrap_or(MetadataExists::NotFound { url: url.0 }),
                                    )
                                })
                            }
                            DownloadRequiredInformation::YoutubePlaylist(
                                YoutubePlaylistDownloadInfo {
                                    video_urls,
                                    playlist_url,
                                },
                            ) => {
                                let uid = playlist_url.uid();
                                let mut metadata_list = Vec::with_capacity(video_urls.len());

                                // TODO: figure out what is supposed to happen here
                                for url in video_urls {
                                    let _audio_identifier =
                                        DownloadRequiredInformation::YoutubeVideo {
                                            url: YoutubeVideoUrl(url),
                                        };

                                    let metadata = get_audio_metadata_from_db(&uid).await.unwrap();
                                    metadata_list.push(metadata);
                                }

                                Ok(MetadataQueryResult::Many(metadata_list))
                            }
                        };

                        query_res
                    }
                    .into_actor(self)
                    .map(move |res, act, ctx| {
                        match res {
                            Ok(MetadataQueryResult::Single(data)) => {
                                let msg = handle_add_single_queue_item(
                                    data,
                                    act,
                                    ctx.address().recipient(),
                                );

                                if let Some(msg) = msg {
                                    act.multicast_result(msg);
                                }
                            }
                            Ok(MetadataQueryResult::Many(_metadata_list)) => {
                                // TODO
                                // create playlist table if not exists

                                // create playlist items for existing items if not exists

                                // create new identifier with missing items & make download request

                                // play found metadata
                            }
                            Err(err_resp) => {
                                act.multicast(err_resp);
                            }
                        }
                    }),
                )
            } // _ => Box::pin(async { Ok(()) }.into_actor(self)),
        }
    }
}

async fn get_audio_metadata_from_db(uid: &str) -> Result<Option<AudioMetaData>, ErrorResponse> {
    sqlx::query_as!(
        AudioMetaData,
        "SELECT name, author, duration, cover_art_url FROM audio_metadata where identifier = $1",
        uid
    )
    .fetch_optional(db_pool())
    .await
    .into_err_resp("")
}

fn handle_add_single_queue_item(
    data: MetadataExists,
    node: &mut AudioNode,
    node_addr: Recipient<NotifyDownloadUpdate>,
) -> Option<Result<AudioNodeInfoStreamMessage, ErrorResponse>> {
    match data {
        MetadataExists::Found { metadata, path } => {
            if let Err(err) = node.player.push_to_queue(AudioPlayerQueueItem {
                metadata,
                locator: path,
            }) {
                log::error!("failed to auto play first song");
                return Some(Err(ErrorResponse {
                    error: format!("failed to auto play first song, ERROR: {err}"),
                }));
            }
        }
        MetadataExists::NotFound { url } => {
            node.downloader_addr.do_send(DownloadAudioRequest {
                addr: node_addr,
                identifier: DownloadRequiredInformation::YoutubeVideo {
                    url: YoutubeVideoUrl(url),
                },
            });

            return None;
        }
    }

    Some(Ok(AudioNodeInfoStreamMessage::Queue(
        extract_queue_metadata(node.player.queue()),
    )))
}

fn handle_remove_queue_item(
    node: &mut AudioNode,
    params: RemoveQueueItemParams,
) -> Result<SerializableQueue, ErrorResponse> {
    let RemoveQueueItemParams { index } = params.clone();

    if let Err(err) = node.player.remove_from_queue(index) {
        log::error!("failed to play correct audio after removing element from queue, MESSAGE: {params:?}, ERROR: {err}");
        return Err(ErrorResponse {
            error: format!("failed to play correct audio after removing element, ERROR: {err}"),
        });
    }

    Ok(extract_queue_metadata(node.player.queue()))
}

fn handle_move_queue_item(node: &mut AudioNode, params: MoveQueueItemParams) -> SerializableQueue {
    let MoveQueueItemParams { old_pos, new_pos } = params;
    node.player.move_queue_item(old_pos, new_pos);

    extract_queue_metadata(node.player.queue())
}

fn extract_queue_metadata<ADL: AudioDataLocator>(
    queue: &[AudioPlayerQueueItem<ADL>],
) -> SerializableQueue {
    queue.iter().map(|item| item.metadata.clone()).collect()
}
