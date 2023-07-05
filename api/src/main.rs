#![allow(dead_code)]

use std::collections::HashMap;
use std::env;
use std::path::Path;
use std::process::Command;
use std::path::PathBuf;

use actix::{Actor, ActorContext, Message, StreamHandler, Addr, Context, Handler, Recipient, AsyncContext};
use actix_cors::Cors;
use actix_web::web::{self, Data};
use actix_web::{get, App, HttpRequest, HttpServer, Responder};
use actix_web_actors::ws;

use anyhow::anyhow;
use audio::AudioPlayer;
use cpal::traits::{DeviceTrait, HostTrait};
use serde::{Deserialize, Serialize};

mod audio;

static AUDIO_DIR: &str = "audio";

pub struct AppData {
    queue_server_addr: Addr<QueueServer>
}

pub struct QueueServer {
    players: HashMap<String, AudioPlayer>,
    sessions: Vec<Recipient<StringMessage>>,
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
    AddPlayer(AddPlayerParams),
    PlayNext(PlayNextParams),
    LoopQueue(LoopQueueParams),
}

#[derive(Debug, Deserialize, Message)]
#[rtype(result = "Result<Vec<PathBuf>, ErrorResponse>")]
#[serde(rename_all = "camelCase")] 
pub struct AddQueueParams {
    pub player_name: String,
    pub title: String,
    pub url: String,
}

#[derive(Debug, Deserialize, Message)]
#[rtype(result = "anyhow::Result<Vec<String>>")]
#[serde(rename_all = "camelCase")] 
pub struct AddPlayerParams {
    pub player_name: String,
}

#[derive(Debug, Deserialize, Message)]
#[rtype(result = "anyhow::Result<()>")]
#[serde(rename_all = "camelCase")] 
pub struct PlayNextParams {
    pub player_name: String,
}

#[derive(Debug, Deserialize, Message)]
#[rtype(result = "anyhow::Result<()>")]
#[serde(rename_all = "camelCase")] 
pub struct LoopQueueParams {
    pub player_name: String,
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
            players: HashMap::new(),
            sessions: Vec::new(),
        }
    }

}

impl Actor for QueueServer {
    type Context = Context<Self>;
}

impl Actor for QueueSession {
    type Context = ws::WebsocketContext<Self>;
}

impl Handler<AddQueueParams> for QueueServer {
    type Result = Result<Vec<PathBuf>, ErrorResponse>;

    fn handle(&mut self, msg: AddQueueParams, _ctx: &mut Self::Context) -> Self::Result {
        let AddQueueParams { player_name, title, url } = msg;
        let Some(player) = self.players.get_mut(&player_name) else {
            // todo log error
            return Err(ErrorResponse { error: format!("no audio player/source with the name {player_name} found") });
        };

        let path = Path::new(AUDIO_DIR).join(&title);
        let mut path_with_ext = path.clone();
        path_with_ext.set_extension("mp3");

        if !path_with_ext.try_exists().unwrap_or(false) {
            let Some(str_path) = path.to_str() else { 
                // todo log error
                return Err( ErrorResponse { error: format!("failed to construct valid path with title: {title}") } );
            };

            if let Err(err) = download_audio(&url, str_path) {
                // todo log error
                return Err( ErrorResponse { error: format!("failed to download video with title: {title}, url: {url}; error: {err}") } );
            }
        }

        player.push_to_queue(path_with_ext);
        Ok(player.queue().to_vec())
    }
}

impl Handler<AddPlayerParams> for QueueServer {
    type Result = anyhow::Result<Vec<String>>;

    fn handle(&mut self, msg: AddPlayerParams, ctx: &mut Self::Context) -> Self::Result {
        let AddPlayerParams { player_name } = msg;

        let host = cpal::default_host();
        let device = host.default_output_device().expect("no output device available");

        let mut supported_configs_range = device.supported_output_configs()
            .expect("error while querying configs");

        let supported_config = supported_configs_range.next()
            .expect("no supported config?!").with_max_sample_rate();
                                                                                      // but should
                                                                                      // definitely look
                                                                                      // into getting
                                                                                      // the proper
                                                                                      // sample rate

        let config = supported_config.into();
        let player = AudioPlayer::new(device, config, Vec::new(), ctx.address().clone());
        self.add_player(player_name, player);

        Ok(self.players.keys().map(|key| key.to_owned()).collect())
   } 
}

impl Handler<PlayNextParams> for QueueServer {
    type Result = anyhow::Result<()>;

   fn handle(&mut self, msg: PlayNextParams, _ctx: &mut Self::Context) -> Self::Result {
       let PlayNextParams { player_name } = msg;

       if let Some(player) = self.players.get_mut(&player_name) {
           player.play_next(player_name)?;
       }

       Ok(())
   } 
}

impl Handler<LoopQueueParams> for QueueServer {
    type Result = anyhow::Result<()>;

   fn handle(&mut self, msg: LoopQueueParams, _ctx: &mut Self::Context) -> Self::Result {
       let LoopQueueParams { player_name, start, end } = msg;

       if let Some(player) = self.players.get_mut(&player_name) {
           player.set_loop(Some((start, end)));
       }

       Ok(())
   } 
}

impl QueueServer {
    fn add_player(&mut self, dev_name: String, player: AudioPlayer) {
        self.players.insert(dev_name, player);
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for QueueSession {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Text(text)) => {
                match serde_json::from_str::<QueueMessage>(&text) {
                    Ok(QueueMessage::AddQueueItem(msg)) => self.server_addr.try_send(msg).unwrap(),
                    Ok(QueueMessage::AddPlayer(msg)) => self.server_addr.try_send(msg).unwrap(),
                    Ok(QueueMessage::PlayNext(msg))=> self.server_addr.try_send(msg).unwrap(),
                    Ok(QueueMessage::LoopQueue(msg)) => self.server_addr.try_send(msg).unwrap(),
                    Err(err) => {
                        // todo log err
                        ctx.text(serde_json::to_string(&ErrorResponse {
                            error: format!("failed to parse message; error: {err}")
                        }).unwrap_or(String::from("{}")))
                    }
                }
            }
            Ok(ws::Message::Close(reason)) => {
                ctx.close(reason);
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
    env::set_var("RUST_LOG", "actix_web=debug,actix_server=info");
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
