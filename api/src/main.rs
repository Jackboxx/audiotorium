#![allow(dead_code)]

use std::time::Duration;
use std::env;
use std::path::Path;
use std::process::Command;
use std::{fs::File, path::PathBuf};

use actix::{Actor, ActorContext, Message, StreamHandler, Addr, Context, Handler, Recipient, AsyncContext};
use actix_cors::Cors;
use actix_web::web::{self, Data};
use actix_web::{get, App, HttpRequest, HttpResponse, HttpServer, Responder};
use actix_web_actors::ws;

use anyhow::anyhow;
use cpal::traits::{DeviceTrait, HostTrait};
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};
use serde::Deserialize;

static AUDIO_DIR: &str = "audio";

struct AppData {
    queue_server_addr: Addr<QueueServer>
}

struct QueueServer {
    queue: AudioQueue,
    sink: Sink,
    sessions: Vec<Recipient<StringMessage>>,
    _stream_handle: OutputStreamHandle,
}

struct QueueSession {
    server_addr: Addr<QueueServer>
}

struct AudioQueue {
    queue: Vec<PathBuf>,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct StringMessage(pub String);

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")] 
enum QueueMessage {
    Add(AddQueueParams)
}

#[derive(Debug, Deserialize, Message)]
#[rtype(result = "anyhow::Result<Vec<PathBuf>>")]
struct AddQueueParams {
    pub title: String,
    pub url: String,
}

impl Default for QueueServer {
    fn default() -> Self {
        let (_stream, stream_handle) = OutputStream::try_default().unwrap();
        let sink = Sink::try_new(&stream_handle).unwrap();

        sink.set_volume(1.0);
        sink.set_speed(1.0);

        Self {
            queue: AudioQueue::default(),
            sink,
            sessions: Vec::new(),
            _stream_handle: stream_handle,
        }
    }
}

impl Default for AudioQueue {
    fn default() -> Self {
        Self { queue: Vec::new() }
    }
}

impl Actor for QueueServer {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        ctx.run_interval(Duration::from_millis(500), |actor, _ctx| actor.update());
    }
}

impl Actor for QueueSession {
    type Context = ws::WebsocketContext<Self>;
}

impl Handler<AddQueueParams> for QueueServer {
    type Result = anyhow::Result<Vec<PathBuf>>;


    fn handle(&mut self, msg: AddQueueParams, _ctx: &mut Self::Context) -> Self::Result {
        let AddQueueParams { title, url } = msg;
        let path = Path::new(AUDIO_DIR).join(&title);

        let mut path_with_ext = path.clone();
        path_with_ext.set_extension("mp3");

        if !path_with_ext.try_exists().unwrap_or(false) {
            let Some(str_path) = path.to_str() else { 
                return Err(anyhow!("failed to construct valid path with title: {title}"));
            };

            match download_audio(&url, str_path) {
                Err(err) => {
                    return Err(anyhow!("{err}"));
                }
                _ => {}
            }
        }

        self.queue.push(&self.sink, path)?;
        Ok(self.queue.queue.clone())
    }
}

impl QueueServer {
    fn update(&mut self) {
        while self.sink.len() < self.queue.queue.len() {
            self.queue.queue.remove(0);

            for session in &self.sessions {
                session.do_send(StringMessage(serde_json::to_string(&self.queue.queue).unwrap()))
            }
        }

        if self.queue.queue.len() < self.sink.len() {
            panic!("sink should not have more elements then queue")
        }
    }
}

impl AudioQueue {
    fn push(&mut self, sink: &Sink, path: PathBuf) -> anyhow::Result<()> {
        println!("adding new");
        let audio = get_audio_from_file(&path)?;
        println!("read file");
        sink.append(audio);
        self.queue.push(path);
        println!("added new");
        Ok(())
    }
}

// impl QueueServer {
//     fn handle_msg(&mut self, msg: QueueWsMessage) -> String {
//         match msg {
//             QueueWsMessage::Add(params) => {
//                 let mut data = self.app_data.lock().unwrap();
//                 let queue = &mut data.queue;

//                 match add_to_queue(queue, params) {
//                     Ok(()) => {
//                         serde_json::to_string(queue).unwrap()
//                     }
//                     Err(_err) => todo!("errors")
//                 }
//             }
//             QueueWsMessage::Play => {
//                 self.sink.lock().unwrap().play();
//                 String::new()
//             }
//         }
//     }
// }

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for QueueSession {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Text(text)) => {
                match serde_json::from_str::<QueueMessage>(&text).unwrap() {
                    QueueMessage::Add(msg) => self.server_addr.try_send(msg).unwrap(),
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

fn get_audio_from_file(path: &PathBuf) -> anyhow::Result<Decoder<File>> {
    let file = File::open(&path)?;
    Ok(Decoder::new(file)?)
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

#[get("/sources")]
async fn get_sources() -> impl Responder {
    let host = cpal::default_host();
    let devices = host.output_devices().unwrap();

    HttpResponse::Ok().json(devices.flat_map(|dev| dev.name()).collect::<Vec<_>>())
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
            .service(get_sources)
            .service(get_con_to_queue)
    })
    .bind(("127.0.0.1", 50051))?
    .run()
    .await
}
