use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::Path;

use actix::{Actor, Message, Context, Handler, AsyncContext, Recipient};

use cpal::traits::{DeviceTrait, HostTrait};
use log::{error, info};
use serde::{Deserialize, Serialize};

use crate::{audio::{AudioSource, PlaybackInfo}, ErrorResponse, AUDIO_DIR, download_audio, session::FilteredPassThroughtMessage};

#[derive(Default)]
pub struct QueueServer {
    sources: HashMap<String, AudioSource>,
    sessions: HashMap<usize, Recipient<FilteredPassThroughtMessage>>
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum QueueServerMessage {
    AddQueueItem(AddQueueItemServerParams),
    ReadQueueItems(ReadQueueServerParams),
    MoveQueueItem(MoveQueueItemServerParams),
    AddSource(AddSourceServerParams),
    ReadSources(ReadSourcesServerParams),
    PlayNext(PlayNextServerParams),
    PlayPrevious(PlayPreviousServerParams),
    PlaySelected(PlaySelectedServerParams),
    LoopQueue(LoopQueueServerParams),
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[allow(clippy::enum_variant_names, dead_code)]
pub enum QueueServerMessageResponse {
    SessionConnectedResponse(ConnectServerResponse),
    AddQueueItemResponse(AddQueueItemServerResponse),
    ReadQueueItemsResponse(ReadQueueServerResponse),
    MoveQueueItemResponse(MoveQueueItemServerResponse),
    AddSourceResponse(AddSourceServerResponse),
    ReadSourcesResponse(ReadSourcesServerResponse),
    PlayNextResponse(PlayNextServerResponse),
    PlayPreviousResponse(PlayPreviousServerResponse),
    PlaySelectedResponse(PlaySelectedServerResponse),
    LoopQueueResponse(LoopQueueServerResponse),
    SendClientQueueInfoResponse(SendClientQueueInfoServerResponse),
}

#[derive(Debug, Clone, Message)]
#[rtype(result = "Result<ConnectServerResponse, ()>")]
pub struct Connect {
    pub addr: Recipient<FilteredPassThroughtMessage>
}

#[derive(Debug, Clone, Serialize)]
pub struct ConnectServerResponse {
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
    pub source_name: String
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SendClientQueueInfoServerResponse {
    pub info: PlaybackInfo,
}

#[derive(Debug, Clone, Deserialize, Message)]
#[rtype(result = "Result<AddQueueItemServerResponse, ErrorResponse>")]
#[serde(rename_all = "camelCase")]
pub struct AddQueueItemServerParams {
    pub source_name: String,
    pub title: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AddQueueItemServerResponse {
    queue: Vec<String>
}

#[derive(Debug, Clone, Deserialize, Message)]
#[rtype(result = "Result<ReadQueueServerResponse, ErrorResponse>")]
#[serde(rename_all = "camelCase")]
pub struct ReadQueueServerParams {
    pub source_name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReadQueueServerResponse {
    queue: Vec<String>
}

#[derive(Debug, Clone, Deserialize, Message)]
#[rtype(result = "Result<MoveQueueItemServerResponse, ErrorResponse>")]
#[serde(rename_all = "camelCase")]
pub struct MoveQueueItemServerParams {
    pub source_name: String, 
    pub old_pos: usize,
    pub new_pos: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct MoveQueueItemServerResponse {
    queue: Vec<String>
}

#[derive(Debug, Clone, Deserialize, Message)]
#[rtype(result = "Result<AddSourceServerResponse, ErrorResponse>")]
#[serde(rename_all = "camelCase")]
pub struct AddSourceServerParams {
    pub source_name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AddSourceServerResponse {
    sources: Vec<String>
}

#[derive(Debug, Clone, Deserialize, Message)]
#[rtype(result = "Result<ReadSourcesServerResponse, ErrorResponse>")]
#[serde(rename_all = "camelCase")]
pub struct ReadSourcesServerParams;

#[derive(Debug, Clone, Serialize)]
pub struct ReadSourcesServerResponse {
    pub sources: Vec<String>
}

#[derive(Debug, Clone, Deserialize, Message)]
#[rtype(result = "Result<PlayNextServerResponse, ErrorResponse>")]
#[serde(rename_all = "camelCase")]
pub struct PlayNextServerParams {
    pub source_name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayNextServerResponse;

#[derive(Debug, Clone, Deserialize, Message)]
#[rtype(result = "Result<PlayPreviousServerResponse, ErrorResponse>")]
#[serde(rename_all = "camelCase")]
pub struct PlayPreviousServerParams {
    pub source_name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayPreviousServerResponse;

#[derive(Debug, Clone, Deserialize, Message)]
#[rtype(result = "Result<PlaySelectedServerResponse, ErrorResponse>")]
#[serde(rename_all = "camelCase")]
pub struct PlaySelectedServerParams {
    pub source_name: String,
    pub index: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaySelectedServerResponse;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoopBounds {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, Deserialize, Message)]
#[rtype(result = "Result<LoopQueueServerResponse, ErrorResponse>")]
#[serde(rename_all = "camelCase")]
pub struct LoopQueueServerParams {
    pub source_name: String,
    pub bounds: Option<LoopBounds>
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoopQueueServerResponse;

impl Actor for QueueServer {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        // check this if weird shit happens just trying stuff here
        ctx.set_mailbox_capacity(64);
        info!("stared new 'QueueServer', CONTEXT: {ctx:?}");
    }
}

impl Handler<Connect> for QueueServer {
    type Result = Result<ConnectServerResponse, ()>;
    fn handle(&mut self, msg: Connect, _ctx: &mut Self::Context) -> Self::Result {
        let Connect { addr } = msg;
        let id = self.sessions.keys().max().unwrap_or(&0) + 1;

        self.sessions.insert(id, addr);

        Ok(ConnectServerResponse { id, 
            sources: self
                .sources
                .keys()
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
       let SendClientQueueInfoParams { source_name } = msg;
       if let Some(source) = self.sources.get(&source_name) {
           for session in &self.sessions {
               let addr = session.1; 
               let msg = 
                   serde_json::to_string(&QueueServerMessageResponse::SendClientQueueInfoResponse(
                           SendClientQueueInfoServerResponse { info: source.playback_info().clone() })
                       ).unwrap_or(String::new());

               addr.do_send(FilteredPassThroughtMessage { 
                   msg,
                   source_name: source_name.clone(),
               });
           }
       } 
   }
}

impl Handler<AddQueueItemServerParams> for QueueServer {
    type Result = Result<AddQueueItemServerResponse, ErrorResponse>;

    fn handle(&mut self, msg: AddQueueItemServerParams, _ctx: &mut Self::Context) -> Self::Result {
        info!("'AddQueueItem' handler received a message, MESSAGE: {msg:?}");

        let AddQueueItemServerParams { source_name , title, url } = msg.clone();
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

    Ok(AddQueueItemServerResponse { 
        queue: source
           .queue()
           .iter()
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

impl Handler<ReadQueueServerParams> for QueueServer {
   type Result = Result<ReadQueueServerResponse, ErrorResponse>; 
   fn handle(&mut self, msg: ReadQueueServerParams, _ctx: &mut Self::Context) -> Self::Result {
        info!("'ReadQueueItems' handler received a message, MESSAGE: {msg:?}");

        let ReadQueueServerParams { source_name } = msg;
        if let Some(source) = self.sources.get(&source_name) {
            Ok(ReadQueueServerResponse { queue: source
            .queue()
            .iter()
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

impl Handler<MoveQueueItemServerParams> for QueueServer  {
   type Result = Result<MoveQueueItemServerResponse, ErrorResponse>; 
   fn handle(&mut self, msg: MoveQueueItemServerParams, _ctx: &mut Self::Context) -> Self::Result {
        info!("'MoveQueueItem' handler received a message, MESSAGE: {msg:?}");

        let MoveQueueItemServerParams { source_name, old_pos, new_pos } = msg;

        if let Some(source) = self.sources.get_mut(&source_name) {
            if old_pos >= source.queue().len() {
                error!("'MoveQueueItem' params out of bounds");
                return Err(ErrorResponse { error: "'oldPos' and 'newPos' out of bounds".to_owned() });
            } 

            source.move_queue_item(old_pos, new_pos);
            Ok(MoveQueueItemServerResponse { 
                queue: source
                   .queue()
                   .iter()
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

impl Handler<AddSourceServerParams> for QueueServer {
    type Result = Result<AddSourceServerResponse, ErrorResponse>;

    fn handle(&mut self, msg: AddSourceServerParams, ctx: &mut Self::Context) -> Self::Result {
        info!("'AddSource' handler received a message, MESSAGE: {msg:?}");

        let AddSourceServerParams { source_name } = msg;

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

        Ok(AddSourceServerResponse { sources: self.sources.keys().map(|key| key.to_owned()).collect() } )
   } 
}

impl Handler<ReadSourcesServerParams> for QueueServer {
   type Result = Result<ReadSourcesServerResponse, ErrorResponse>; 

   fn handle(&mut self, _msg: ReadSourcesServerParams, _ctx: &mut Self::Context) -> Self::Result {
       Ok(ReadSourcesServerResponse { 
           sources: self
               .sources
               .keys()
               .map(|k| k.to_owned())
               .collect()
       })
   }
}

impl Handler<PlayNextServerParams> for QueueServer {
    type Result = Result<PlayNextServerResponse, ErrorResponse>;

   fn handle(&mut self, msg: PlayNextServerParams, _ctx: &mut Self::Context) -> Self::Result {
       info!("'PlayNext' handler received a message, MESSAGE: {msg:?}");

       let PlayNextServerParams { source_name } = msg.clone();

       if let Some(source) = self.sources.get_mut(&source_name) {
           if let Err(err) = source.play_next(source_name) {
            error!("failed to play next audio, MESSAGE: {msg:?}, ERROR: {err}");
            return Err(ErrorResponse { error: format!("failed to play next audio, ERROR: {err}") });
           }
       } else {
            error!("no audio source with the name {source_name} found, SOURCES: {:?}", self.sources.keys());
            return Err(ErrorResponse { error: format!("no audio source with the name {source_name} found") });
       }

       Ok(PlayNextServerResponse)
   } 
}

impl Handler<PlayPreviousServerParams> for QueueServer {
    type Result = Result<PlayPreviousServerResponse, ErrorResponse>;

   fn handle(&mut self, msg: PlayPreviousServerParams, _ctx: &mut Self::Context) -> Self::Result {
       info!("'PlayPrevious' handler received a message, MESSAGE: {msg:?}");

       let PlayPreviousServerParams { source_name } = msg.clone();

       if let Some(source) = self.sources.get_mut(&source_name) {
           if let Err(err) = source.play_prev(source_name) {
            error!("failed to play previous audio, MESSAGE: {msg:?}, ERROR: {err}");
            return Err(ErrorResponse { error: format!("failed to play previous audio, ERROR: {err}") });
           }
       } else {
            error!("no audio source with the name {source_name} found, SOURCES: {:?}", self.sources.keys());
            return Err(ErrorResponse { error: format!("no audio source with the name {source_name} found") });
       }

       Ok(PlayPreviousServerResponse)
   } 
}

impl Handler<PlaySelectedServerParams> for QueueServer {
    type Result = Result<PlaySelectedServerResponse, ErrorResponse>;

   fn handle(&mut self, msg: PlaySelectedServerParams, _ctx: &mut Self::Context) -> Self::Result {
       info!("'PlayPrevious' handler received a message, MESSAGE: {msg:?}");

       let PlaySelectedServerParams { index, source_name } = msg.clone();

       if let Some(source) = self.sources.get_mut(&source_name) {
           if let Err(err) = source.play_selected(index, source_name) {
            error!("failed to play selected audio, MESSAGE: {msg:?}, ERROR: {err}");
            return Err(ErrorResponse { error: format!("failed to play track with index {index} audio, ERROR: {err}") });
           }
       } else {
            error!("no audio source with the name {source_name} found, SOURCES: {:?}", self.sources.keys());
            return Err(ErrorResponse { error: format!("no audio source with the name {source_name} found") });
       }

       Ok(PlaySelectedServerResponse)
   } 
}

impl Handler<LoopQueueServerParams> for QueueServer {
    type Result = Result<LoopQueueServerResponse, ErrorResponse>;

   fn handle(&mut self, msg: LoopQueueServerParams, _ctx: &mut Self::Context) -> Self::Result {
       info!("'LoopQueue' handler received a message, MESSAGE: {msg:?}");

       let LoopQueueServerParams { source_name, bounds } = msg;

       if let Some(source) = self.sources.get_mut(&source_name) {
           source.set_loop(bounds);
       } else {
            error!("no audio source with the name {source_name} found, SOURCES: {:?}", self.sources.keys());
            return Err(ErrorResponse { error: format!("no audio source/source with the name {source_name} found") });
       }

       Ok(LoopQueueServerResponse)
   } 
}

impl QueueServer {
    fn add_source(&mut self, source_name: String, source: AudioSource) {
        self.sources.insert(source_name, source);
    }
}
