use std::collections::HashMap;
use std::env;
use std::path::Path;
use std::process::Command;
use std::path::PathBuf;

use actix::{Actor, ActorContext, Message, StreamHandler, Addr, Context, Handler, AsyncContext, WrapFuture, ActorFutureExt};
use actix_cors::Cors;
use actix_web::web::{self, Data};
use actix_web::{get, App, HttpRequest, HttpServer, Responder};
use actix_web_actors::ws;

use anyhow::anyhow;
use audio::AudioSource;
use cpal::traits::{DeviceTrait, HostTrait};
use log::{error, info};
use serde::{Deserialize, Serialize};

mod audio;

static AUDIO_DIR: &str = "audio";

pub struct AppData {
    queue_server_addr: Addr<QueueServer>
}

pub struct QueueServer {
    sources: HashMap<String, AudioSource>,
}

pub struct QueueSession {
    server_addr: Addr<QueueServer>
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct StringMessage(pub String);

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")] 
pub enum QueueMessage {
    AddQueueItem(AddQueueParams),
    AddSource(AddSourceParams),
    PlayNext(PlayNextParams),
    LoopQueue(LoopQueueParams),
}

#[derive(Debug, Deserialize, Message)]
#[rtype(result = "Result<Vec<PathBuf>, ErrorResponse>")]
#[serde(rename_all = "camelCase")] 
pub struct AddQueueParams {
    pub source_name: String,
    pub title: String,
    pub url: String,
}

#[derive(Debug, Deserialize, Message)]
#[rtype(result = "Result<Vec<String>, ErrorResponse>")]
#[serde(rename_all = "camelCase")] 
pub struct AddSourceParams {
    pub source_name: String,
}

#[derive(Debug, Deserialize, Message)]
#[rtype(result = "Result<(), ErrorResponse>")]
#[serde(rename_all = "camelCase")] 
pub struct PlayNextParams {
    pub source_name: String,
}

#[derive(Debug, Deserialize, Message)]
#[rtype(result = "Result<(), ErrorResponse>")]
#[serde(rename_all = "camelCase")] 
pub struct LoopQueueParams {
    pub source_name: String,
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")] 
pub struct ErrorResponse {
    error: String,
}

impl Default for QueueServer {
    fn default() -> Self {
        Self {
            sources: HashMap::new(),
        }
    }

}

impl Actor for QueueServer {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        info!("stared new 'QueueServer', CONTEXT: {ctx:?}");
    }
}

impl Actor for QueueSession {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        info!("stared new 'QueueSession'");
    }
}

impl Handler<AddQueueParams> for QueueServer {
    type Result = Result<Vec<PathBuf>, ErrorResponse>;

    fn handle(&mut self, msg: AddQueueParams, _ctx: &mut Self::Context) -> Self::Result {
        info!("'AddQueueItem' handler received a message, MESSAGE: {msg:?}");

        let AddQueueParams { source_name , title, url } = msg;
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

        source.push_to_queue(path_with_ext);
        Ok(source.queue().to_vec())
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
        let source = AudioSource::new(device, config, Vec::new(), ctx.address().clone());
        self.add_source(source_name, source);

        Ok(self.sources.keys().map(|key| key.to_owned()).collect())
   } 
}

impl Handler<PlayNextParams> for QueueServer {
    type Result = Result<(), ErrorResponse>;

   fn handle(&mut self, msg: PlayNextParams, _ctx: &mut Self::Context) -> Self::Result {
       info!("'PlayNext' handler received a message, MESSAGE: {msg:?}");

       let PlayNextParams { source_name } = msg;

       if let Some(source) = self.sources.get_mut(&source_name) {
           if let Err(err) = source.play_next(source_name) {
            return Err(ErrorResponse { error: format!("failed to play next audio, ERROR: {err}") });
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

       let LoopQueueParams { source_name, start, end } = msg;

       if let Some(source) = self.sources.get_mut(&source_name) {
           source.set_loop(Some((start, end)));
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

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for QueueSession {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match &msg {
            Ok(ws::Message::Text(text)) => {
                match serde_json::from_str::<QueueMessage>(&text) {
                    Ok(QueueMessage::AddQueueItem(msg)) => {
                        let addr = self.server_addr.clone();
                        let fut = async move {
                            addr.send(msg).await
                        }.into_actor(self).map(|result, _, ctx| {
                            match result {
                                Ok(resp) => match resp {
                                    Ok(queue) => {
                                        ctx.text(serde_json::to_string(&queue).unwrap_or("[]".to_owned()));
                                    }
                                    Err(err_resp) => {
                                        ctx.text(serde_json::to_string(&err_resp).unwrap_or("{}".to_owned()));
                                    }
                                },
                                Err(err) => {
                                    error!("queue server didn't responde to 'AddQueueItem' message, ERROR: {err}");
                                    ctx.text(serde_json::to_string(&ErrorResponse {
                                        error: format!("server failed to responde to message, ERROR: {err}")
                                    }).unwrap_or("{}".to_owned()));
                                }
                            }
                        });

                        ctx.spawn(fut);
                    } 
                    Ok(QueueMessage::AddSource(msg)) => {
                        let addr = self.server_addr.clone();
                        let fut = async move {
                            addr.send(msg).await
                        }.into_actor(self).map(|result, _, ctx| {
                            match result {
                                Ok(resp) => match resp {
                                    Ok(sources) => {
                                        ctx.text(serde_json::to_string(&sources).unwrap_or("[]".to_owned()));
                                    }
                                    Err(err_resp) => {
                                        ctx.text(serde_json::to_string(&err_resp).unwrap_or("{}".to_owned()));
                                    }
                                },
                                Err(err) => {
                                    error!("queue server didn't responde to 'AddSource' message, ERROR: {err}");
                                    ctx.text(serde_json::to_string(&ErrorResponse {
                                        error: format!("server failed to responde to message, ERROR: {err}")
                                    }).unwrap_or("{}".to_owned()));
                                }
                            }
                        });

                        ctx.spawn(fut);
                    }
                    Ok(QueueMessage::PlayNext(msg))=> {
                        let addr = self.server_addr.clone();
                        let fut = async move {
                            addr.send(msg).await
                        }.into_actor(self).map(|result, _, ctx| {
                            match result {
                                Ok(resp) => match resp {
                                    Ok(()) => {}
                                    Err(err_resp) => {
                                        ctx.text(serde_json::to_string(&err_resp).unwrap_or("{}".to_owned()));
                                    }
                                },
                                Err(err) => {
                                    error!("queue server didn't responde to 'PlayNext' message, ERROR: {err}");
                                    ctx.text(serde_json::to_string(&ErrorResponse {
                                        error: format!("server failed to responde to message, ERROR: {err}")
                                    }).unwrap_or("{}".to_owned()));
                                }
                            }
                        });

                        ctx.spawn(fut);
                    }
                    Ok(QueueMessage::LoopQueue(msg)) => {
                        let addr = self.server_addr.clone();
                        let fut = async move {
                            addr.send(msg).await
                        }.into_actor(self).map(|result, _, ctx| {
                            match result {
                                Ok(resp) => match resp {
                                    Ok(()) => {}
                                    Err(err_resp) => {
                                        ctx.text(serde_json::to_string(&err_resp).unwrap_or("{}".to_owned()));
                                    }
                                },
                                Err(err) => {
                                    error!("queue server didn't responde to 'PlayNext' message, ERROR: {err}");
                                    ctx.text(serde_json::to_string(&ErrorResponse {
                                        error: format!("server failed to responde to message, ERROR: {err}")
                                    }).unwrap_or("{}".to_owned()));
                                }
                            }
                        });

                        ctx.spawn(fut);
                    }
                    Err(err) => {
                        error!("failed to parse message, MESSAGE: {msg:?}, ERROR: {err}");

                        ctx.text(serde_json::to_string(&ErrorResponse {
                            error: format!("failed to parse message, ERROR: {err}"),
                        }).unwrap_or(String::from("{}")))
                    }
                }
            }
            Ok(ws::Message::Close(reason)) => {
                ctx.close(reason.clone());
                ctx.stop();
            }
            _ => {}
        }
    }
}

fn download_audio(url: &str, download_location: &str) -> anyhow::Result<()> {
    let out = Command::new("yt-dlp")
        .args([
            "-f",
            "bestaudio",
            "-x",
            "--audio-format",
            "mp3",
            "-o",
            download_location,
            url,
        ])
        .output()?;

    if out.status.code().unwrap_or(1) != 0 {
        dbg!(out);
        return Err(anyhow!(""));
    }

    Ok(())
}

#[get("/queue")]
async fn get_con_to_queue(
    data: Data<AppData>,
    req: HttpRequest,
    stream: web::Payload,
) -> impl Responder {
    ws::start(QueueSession { server_addr: data.queue_server_addr.clone() }, &req, stream)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env::set_var("RUST_LOG", "info");
    env_logger::init();

    let queue_server = QueueServer::default();
    let server_addr = queue_server.start();

    let data = Data::new(AppData {queue_server_addr: server_addr});
    HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allow_any_header();

        App::new()
            .app_data(data.clone())
            .wrap(cors)
            .service(get_con_to_queue)
    })
    .bind(("127.0.0.1", 50051))?
    .run()
    .await
}
