use audio::AudioSource;
use cpal::traits::{DeviceTrait, HostTrait};
use cpal::SampleRate;
use dotenv;
use std::env;

use actix::{Actor, Addr};
use actix_cors::Cors;
use actix_web::web::{self, Data};
use actix_web::{get, App, HttpRequest, HttpServer, Responder};
use actix_web_actors::ws;

use downloader::AudioDownloader;
use serde::{Deserialize, Serialize};
use server::QueueServer;

use crate::session::QueueSession;

mod audio;
mod downloader;
mod server;
mod session;

pub static AUDIO_DIR: &str = "audio";

pub struct AppData {
    queue_server_addr: Addr<QueueServer>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorResponse {
    error: String,
}

#[get("/queue")]
async fn get_con_to_queue(
    data: Data<AppData>,
    req: HttpRequest,
    stream: web::Payload,
) -> impl Responder {
    ws::start(
        QueueSession::new(data.queue_server_addr.clone()),
        &req,
        stream,
    )
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv::dotenv().ok();

    let addr = if cfg!(not(debug_assertions)) {
        dotenv::var("API_ADDRESS_PROD")
            .expect("environment variable 'API_ADDRESS_PROD' should exist for production builds")
    } else {
        dotenv::var("API_ADDRESS")
            .expect("environment variable 'API_ADDRESS' should exist for debug builds")
    };

    env::set_var("RUST_LOG", "info");
    env_logger::init();

    let downloader = AudioDownloader::default();
    let downloader_addr = downloader.start();

    let queue_server = QueueServer::new(downloader_addr);
    let server_addr = queue_server.start();

    let host = cpal::default_host();
    let device = host
        .output_devices()
        .expect("no output device available")
        .find(|dev| dev.name().expect("device has no name") == "living_room")
        .expect("no device found");

    let mut supported_configs_range = device
        .supported_output_configs()
        .expect("error while querying configs");

    let supported_config = supported_configs_range
        .next()
        .expect("no supported config?!");

    let config = supported_config.with_sample_rate(SampleRate(48000)).into();
    let mut source = AudioSource::new(device, config, Vec::new(), server_addr);

    source
        .push_to_queue("audio/test.mp3".into(), "living_room".to_string())
        .expect("oops something did went go fuck itself");

    loop {}

    // let data = Data::new(AppData {
    //     queue_server_addr: server_addr,
    // });

    // HttpServer::new(move || {
    //     let cors = Cors::default()
    //         .allow_any_origin()
    //         .allow_any_method()
    //         .allow_any_header();

    //     App::new()
    //         .app_data(data.clone())
    //         .wrap(cors)
    //         .service(get_con_to_queue)
    // })
    // .bind((addr, 50051))?
    // .run()
    // .await
}
