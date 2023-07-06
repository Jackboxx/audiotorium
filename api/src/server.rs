use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::Path;

use actix::{Actor, Message, Context, Handler, AsyncContext};

use cpal::traits::{DeviceTrait, HostTrait};
use log::{error, info};
use serde::Deserialize;

use crate::{audio::AudioSource, ErrorResponse, AUDIO_DIR, download_audio};

#[derive(Default)]
pub struct QueueServer {
    sources: HashMap<String, AudioSource>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum QueueMessage {
    AddQueueItem(AddQueueParams),
    AddSource(AddSourceParams),
    PlayNext(PlayNextParams),
    PlayPrevious(PlayPreviousParams),
    PlaySelected(PlaySelectedParams),
    LoopQueue(LoopQueueParams),
}

#[derive(Debug, Clone, Deserialize, Message)]
#[rtype(result = "Result<Vec<String>, ErrorResponse>")]
#[serde(rename_all = "camelCase")]
pub struct AddQueueParams {
    pub source_name: String,
    pub title: String,
    pub url: String,
}

#[derive(Debug, Clone, Deserialize, Message)]
#[rtype(result = "Result<Vec<String>, ErrorResponse>")]
#[serde(rename_all = "camelCase")]
pub struct AddSourceParams {
    pub source_name: String,
}

#[derive(Debug, Clone, Deserialize, Message)]
#[rtype(result = "Result<(), ErrorResponse>")]
#[serde(rename_all = "camelCase")]
pub struct PlayNextParams {
    pub source_name: String,
}

#[derive(Debug, Clone, Deserialize, Message)]
#[rtype(result = "Result<(), ErrorResponse>")]
#[serde(rename_all = "camelCase")]
pub struct PlayPreviousParams {
    pub source_name: String,
}

#[derive(Debug, Clone, Deserialize, Message)]
#[rtype(result = "Result<(), ErrorResponse>")]
#[serde(rename_all = "camelCase")]
pub struct PlaySelectedParams {
    pub source_name: String,
    pub index: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoopBounds {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, Deserialize, Message)]
#[rtype(result = "Result<(), ErrorResponse>")]
#[serde(rename_all = "camelCase")]
pub struct LoopQueueParams {
    pub source_name: String,
    pub bounds: Option<LoopBounds>
}

impl Actor for QueueServer {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        info!("stared new 'QueueServer', CONTEXT: {ctx:?}");
    }
}

impl Handler<AddQueueParams> for QueueServer {
    type Result = Result<Vec<String>, ErrorResponse>;

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

    Ok(source
       .queue()
       .into_iter()
       .map(|path| path
            .file_stem()
            .unwrap_or(OsStr::new(""))
            .to_str()
            .map(|str| str.to_owned())
            .unwrap_or(String::new())
        ).collect())
    }
}

impl Handler<AddSourceParams> for QueueServer {
    type Result = Result<Vec<String>, ErrorResponse>;

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

        Ok(self.sources.keys().map(|key| key.to_owned()).collect())
   } 
}

impl Handler<PlayNextParams> for QueueServer {
    type Result = Result<(), ErrorResponse>;

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

       Ok(())
   } 
}

impl Handler<PlayPreviousParams> for QueueServer {
    type Result = Result<(), ErrorResponse>;

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

       Ok(())
   } 
}

impl Handler<PlaySelectedParams> for QueueServer {
    type Result = Result<(), ErrorResponse>;

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

       Ok(())
   } 
}


impl Handler<LoopQueueParams> for QueueServer {
    type Result = Result<(), ErrorResponse>;

   fn handle(&mut self, msg: LoopQueueParams, _ctx: &mut Self::Context) -> Self::Result {
       info!("'LoopQueue' handler received a message, MESSAGE: {msg:?}");

       let LoopQueueParams { source_name, bounds } = msg;

       if let Some(source) = self.sources.get_mut(&source_name) {
           source.set_loop(bounds);
       } else {
            error!("no audio source with the name {source_name} found, SOURCES: {:?}", self.sources.keys());
            return Err(ErrorResponse { error: format!("no audio source/source with the name {source_name} found") });
       }

       Ok(())
   } 
}

impl QueueServer {
    fn add_source(&mut self, source_name: String, source: AudioSource) {
        self.sources.insert(source_name, source);
    }
}

