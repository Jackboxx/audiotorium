use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::Path;

use actix::{Actor, Message, Context, Handler, AsyncContext, Recipient};

use cpal::traits::{DeviceTrait, HostTrait};
use log::{error, info};
use serde::{Deserialize, Serialize};

use crate::{audio::{AudioSource, PlaybackInfo}, ErrorResponse, AUDIO_DIR, download_audio, session::PassThroughtMessage};

#[derive(Default)]
pub struct QueueServer {
    sources: HashMap<String, AudioSource>,
    sessions: HashMap<usize, Recipient<PassThroughtMessage>>
}

#[derive(Debug, Clone, Message)]
#[rtype(result = "Result<ConnectResponse, ()>")]
pub struct Connect {
    pub addr: Recipient<PassThroughtMessage>
}

#[derive(Debug, Clone, Serialize)]
pub struct ConnectResponse {
    pub id: usize,
    pub sources: Vec<String>,
}

#[derive(Debug, Clone, Message)]
#[rtype(result = "()")]
pub struct Disconnect {
    pub id: usize
}

#[derive(Debug, Clone, Deserialize, Message)]
#[rtype(result = "()")]
#[serde(rename_all = "camelCase")]
pub struct SendClientQueueInfoParams {
    pub info: PlaybackInfo,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SendClientQueueInfoResponseParams {
    pub info: PlaybackInfo,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum QueueServerMessage {
    AddQueueItem(AddQueueParams),
    ReadQueueItems(ReadQueueParams),
    MoveQueueItem(MoveQueueItemParams),
    AddSource(AddSourceParams),
    PlayNext(PlayNextParams),
    PlayPrevious(PlayPreviousParams),
    PlaySelected(PlaySelectedParams),
    LoopQueue(LoopQueueParams),
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum QueueServerMessageResponse {
    SessionConnectedResponse(ConnectResponse),
    AddQueueItemResponse(AddQueueResponseParams),
    ReadQueueItemsResponse(ReadQueueResponseParams),
    MoveQueueItemResponse(MoveQueueItemResponseParams),
    AddSourceResponse(AddSourceResponseParams),
    PlayNextResponse(PlayNextResponseParams),
    PlayPreviousResponse(PlayPreviousResponseParams),
    PlaySelectedResponse(PlaySelectedResponseParams),
    LoopQueueResponse(LoopQueueResponseParams),
    SendClientQueueInfoResponse(SendClientQueueInfoResponseParams),
}

#[derive(Debug, Clone, Deserialize, Message)]
#[rtype(result = "Result<AddQueueResponseParams, ErrorResponse>")]
#[serde(rename_all = "camelCase")]
pub struct AddQueueParams {
    pub source_name: String,
    pub title: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AddQueueResponseParams {
    queue: Vec<String>
}

#[derive(Debug, Clone, Deserialize, Message)]
#[rtype(result = "Result<ReadQueueResponseParams, ErrorResponse>")]
#[serde(rename_all = "camelCase")]
pub struct ReadQueueParams {
    pub source_name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReadQueueResponseParams {
    queue: Vec<String>
}

#[derive(Debug, Clone, Deserialize, Message)]
#[rtype(result = "Result<MoveQueueItemResponseParams, ErrorResponse>")]
#[serde(rename_all = "camelCase")]
pub struct MoveQueueItemParams {
    source_name: String, 
    old_pos: usize,
    new_pos: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct MoveQueueItemResponseParams {
    queue: Vec<String>
}

#[derive(Debug, Clone, Deserialize, Message)]
#[rtype(result = "Result<AddSourceResponseParams, ErrorResponse>")]
#[serde(rename_all = "camelCase")]
pub struct AddSourceParams {
    pub source_name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AddSourceResponseParams {
    sources: Vec<String>
}

#[derive(Debug, Clone, Deserialize, Message)]
#[rtype(result = "Result<PlayNextResponseParams, ErrorResponse>")]
#[serde(rename_all = "camelCase")]
pub struct PlayNextParams {
    pub source_name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayNextResponseParams;

#[derive(Debug, Clone, Deserialize, Message)]
#[rtype(result = "Result<PlayPreviousResponseParams, ErrorResponse>")]
#[serde(rename_all = "camelCase")]
pub struct PlayPreviousParams {
    pub source_name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayPreviousResponseParams;

#[derive(Debug, Clone, Deserialize, Message)]
#[rtype(result = "Result<PlaySelectedResponseParams, ErrorResponse>")]
#[serde(rename_all = "camelCase")]
pub struct PlaySelectedParams {
    pub source_name: String,
    pub index: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaySelectedResponseParams;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoopBounds {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, Deserialize, Message)]
#[rtype(result = "Result<LoopQueueResponseParams, ErrorResponse>")]
#[serde(rename_all = "camelCase")]
pub struct LoopQueueParams {
    pub source_name: String,
    pub bounds: Option<LoopBounds>
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoopQueueResponseParams;

impl Actor for QueueServer {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        // check this if weird shit happens just trying stuff here
        ctx.set_mailbox_capacity(64);
        info!("stared new 'QueueServer', CONTEXT: {ctx:?}");
    }
}

impl Handler<Connect> for QueueServer {
    type Result = Result<ConnectResponse, ()>;
    fn handle(&mut self, msg: Connect, _ctx: &mut Self::Context) -> Self::Result {
        let Connect { addr } = msg;
        let id = self.sessions.keys().max().unwrap_or(&0) + 1;

        self.sessions.insert(id, addr);

        Ok(ConnectResponse { id, 
            sources: self
                .sources
                .keys()
                .into_iter()
                .map(|key| key.to_owned())
                .collect()
        })
    }
}

impl Handler<Disconnect> for QueueServer {
    type Result = ();
    fn handle(&mut self, msg: Disconnect, _ctx: &mut Self::Context) -> Self::Result {
        let Disconnect { id } = msg;
        self.sessions.remove(&id);
    }
}

impl Handler<SendClientQueueInfoParams> for QueueServer {
   type Result = ();

   fn handle(&mut self, msg: SendClientQueueInfoParams, _ctx: &mut Self::Context) -> Self::Result {
       let SendClientQueueInfoParams { info } = msg;
       for session in &self.sessions {
           let addr = session.1; 
           let msg_str = 
               serde_json::to_string(&QueueServerMessageResponse::SendClientQueueInfoResponse(
                       SendClientQueueInfoResponseParams { info: info.clone() })
                   ).unwrap_or(String::new());

           addr.do_send(PassThroughtMessage(msg_str));
       }
   }
}

impl Handler<AddQueueParams> for QueueServer {
    type Result = Result<AddQueueResponseParams, ErrorResponse>;

    fn handle(&mut self, msg: AddQueueParams, _ctx: &mut Self::Context) -> Self::Result {
        info!("'AddQueueItem' handler received a message, MESSAGE: {msg:?}");

        let AddQueueParams { source_name , title, url } = msg.clone();
        let Some(source) = self.sources.get_mut(&source_name) else {
            error!("no audio source with the name {source_name} found, SOURCES: {:?}", self.sources.keys());
            return Err(ErrorResponse { error: format!("no audio source/source with the name {source_name} found") });
        };

        let path = Path::new(AUDIO_DIR).join(&title);
        let mut path_with_ext = path.clone();
        path_with_ext.set_extension("mp3");

        if !path_with_ext.try_exists().unwrap_or(false) {
            let Some(str_path) = path.to_str() else { 
                error!("path {path:?} can't be converted to a string");
                return Err( ErrorResponse { error: format!("failed to construct valid path with title: {title}") } );
            };

            if let Err(err) = download_audio(&url, str_path) {
                error!("failed to download video, URL: {url}, ERROR: {err}");
                return Err( ErrorResponse { error: format!("failed to download video with title: {title}, url: {url}, ERROR: {err}") } );
            }
        }

    if let Err(err) = source.push_to_queue(path_with_ext, source_name) {
        error!("failed to auto play first song, MESSAGE: {msg:?}, ERROR: {err}");
        return Err( ErrorResponse { 
            error: format!("failed to auto play first song, ERROR: {err}") 
        });
    }

    Ok(AddQueueResponseParams { 
        queue: source
           .queue()
           .into_iter()
           .map(|path| path
                .file_stem()
                .unwrap_or(OsStr::new(""))
                .to_str()
                .map(|str| str.to_owned())
                .unwrap_or(String::new())
            ).collect()
        })
    }
}

impl Handler<ReadQueueParams> for QueueServer {
   type Result = Result<ReadQueueResponseParams, ErrorResponse>; 
   fn handle(&mut self, msg: ReadQueueParams, _ctx: &mut Self::Context) -> Self::Result {
        info!("'ReadQueueItems' handler received a message, MESSAGE: {msg:?}");

        let ReadQueueParams { source_name } = msg;
        if let Some(source) = self.sources.get(&source_name) {
            Ok(ReadQueueResponseParams { queue: source
            .queue()
            .into_iter()
            .map(|path| path
                .file_stem()
                .unwrap_or(OsStr::new(""))
                .to_str()
                .map(|str| str.to_owned())
                .unwrap_or(String::new())
            ).collect()
        })
       } else {
            error!("no audio source with the name {source_name} found, SOURCES: {:?}", self.sources.keys());
            Err(ErrorResponse { error: format!("no audio source with the name {source_name} found") })
       }
   }
}

impl Handler<MoveQueueItemParams> for QueueServer  {
   type Result = Result<MoveQueueItemResponseParams, ErrorResponse>; 
   fn handle(&mut self, msg: MoveQueueItemParams, _ctx: &mut Self::Context) -> Self::Result {
        info!("'MoveQueueItem' handler received a message, MESSAGE: {msg:?}");

        let MoveQueueItemParams { source_name, old_pos, new_pos } = msg;

        if let Some(source) = self.sources.get_mut(&source_name) {
            if old_pos >= source.queue().len() || new_pos < 0 {
                error!("'MoveQueueItem' params out of bounds");
                return Err(ErrorResponse { error: format!("'oldPos' and 'newPos' out of bounds") });
            } 

            source.move_queue_item(old_pos, new_pos);
            Ok(MoveQueueItemResponseParams { 
                queue: source
                   .queue()
                   .into_iter()
                   .map(|path| path
                        .file_stem()
                        .unwrap_or(OsStr::new(""))
                        .to_str()
                        .map(|str| str.to_owned())
                        .unwrap_or(String::new())
                    ).collect()
                })
        } else {
            error!("no audio source with the name {source_name} found, SOURCES: {:?}", self.sources.keys());
            Err(ErrorResponse { error: format!("no audio source with the name {source_name} found") })
        }        
    }        
}

impl Handler<AddSourceParams> for QueueServer {
    type Result = Result<AddSourceResponseParams, ErrorResponse>;

    fn handle(&mut self, msg: AddSourceParams, ctx: &mut Self::Context) -> Self::Result {
        info!("'AddSource' handler received a message, MESSAGE: {msg:?}");

        let AddSourceParams { source_name } = msg;

        let host = cpal::default_host();
        let device = host.default_output_device().expect("no output device available");

        let mut supported_configs_range = device.supported_output_configs()
            .expect("error while querying configs");

        let supported_config = supported_configs_range.next()
            .expect("no supported config?!").with_sample_rate(cpal::SampleRate(16384 * 6));
                                                                                      // but should
                                                                                      // definitely look
                                                                                      // into getting
                                                                                      // the proper
                                                                                      // sample rate

        let config = supported_config.into();
        let source = AudioSource::new(device, config, Vec::new(), ctx.address());
        self.add_source(source_name, source);

        Ok(AddSourceResponseParams { sources: self.sources.keys().map(|key| key.to_owned()).collect() } )
   } 
}

impl Handler<PlayNextParams> for QueueServer {
    type Result = Result<PlayNextResponseParams, ErrorResponse>;

   fn handle(&mut self, msg: PlayNextParams, _ctx: &mut Self::Context) -> Self::Result {
       info!("'PlayNext' handler received a message, MESSAGE: {msg:?}");

       let PlayNextParams { source_name } = msg.clone();

       if let Some(source) = self.sources.get_mut(&source_name) {
           if let Err(err) = source.play_next(source_name) {
            error!("failed to play next audio, MESSAGE: {msg:?}, ERROR: {err}");
            return Err(ErrorResponse { error: format!("failed to play next audio, ERROR: {err}") });
           }
       } else {
            error!("no audio source with the name {source_name} found, SOURCES: {:?}", self.sources.keys());
            return Err(ErrorResponse { error: format!("no audio source with the name {source_name} found") });
       }

       Ok(PlayNextResponseParams)
   } 
}

impl Handler<PlayPreviousParams> for QueueServer {
    type Result = Result<PlayPreviousResponseParams, ErrorResponse>;

   fn handle(&mut self, msg: PlayPreviousParams, _ctx: &mut Self::Context) -> Self::Result {
       info!("'PlayPrevious' handler received a message, MESSAGE: {msg:?}");

       let PlayPreviousParams { source_name } = msg.clone();

       if let Some(source) = self.sources.get_mut(&source_name) {
           if let Err(err) = source.play_prev(source_name) {
            error!("failed to play previous audio, MESSAGE: {msg:?}, ERROR: {err}");
            return Err(ErrorResponse { error: format!("failed to play previous audio, ERROR: {err}") });
           }
       } else {
            error!("no audio source with the name {source_name} found, SOURCES: {:?}", self.sources.keys());
            return Err(ErrorResponse { error: format!("no audio source with the name {source_name} found") });
       }

       Ok(PlayPreviousResponseParams)
   } 
}

impl Handler<PlaySelectedParams> for QueueServer {
    type Result = Result<PlaySelectedResponseParams, ErrorResponse>;

   fn handle(&mut self, msg: PlaySelectedParams, _ctx: &mut Self::Context) -> Self::Result {
       info!("'PlayPrevious' handler received a message, MESSAGE: {msg:?}");

       let PlaySelectedParams { index, source_name } = msg.clone();

       if let Some(source) = self.sources.get_mut(&source_name) {
           if let Err(err) = source.play_selected(index, source_name) {
            error!("failed to play selected audio, MESSAGE: {msg:?}, ERROR: {err}");
            return Err(ErrorResponse { error: format!("failed to play track with index {index} audio, ERROR: {err}") });
           }
       } else {
            error!("no audio source with the name {source_name} found, SOURCES: {:?}", self.sources.keys());
            return Err(ErrorResponse { error: format!("no audio source with the name {source_name} found") });
       }

       Ok(PlaySelectedResponseParams)
   } 
}

impl Handler<LoopQueueParams> for QueueServer {
    type Result = Result<LoopQueueResponseParams, ErrorResponse>;

   fn handle(&mut self, msg: LoopQueueParams, _ctx: &mut Self::Context) -> Self::Result {
       info!("'LoopQueue' handler received a message, MESSAGE: {msg:?}");

       let LoopQueueParams { source_name, bounds } = msg;

       if let Some(source) = self.sources.get_mut(&source_name) {
           source.set_loop(bounds);
       } else {
            error!("no audio source with the name {source_name} found, SOURCES: {:?}", self.sources.keys());
            return Err(ErrorResponse { error: format!("no audio source/source with the name {source_name} found") });
       }

       Ok(LoopQueueResponseParams)
   } 
}

impl QueueServer {
    fn add_source(&mut self, source_name: String, source: AudioSource) {
        self.sources.insert(source_name, source);
    }
}
