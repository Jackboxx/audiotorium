use brain::AudioBrain;
use commands::node_commands::receive_node_cmd;
use log::LevelFilter;

use actix::{Actor, Addr};
use actix_cors::Cors;
use actix_web::web::{self, Data};
use actix_web::{get, App, HttpRequest, HttpServer, Responder};
use actix_web_actors::ws;

use downloader::AudioDownloader;
use serde::{Deserialize, Serialize};
use streams::node_streams::get_node_stream;

use crate::brain_session::AudioBrainSession;

mod commands;
mod streams;

mod audio_item;
mod audio_player;
mod brain;
mod brain_session;
mod downloader;
mod node;
mod node_session;
mod utils;

pub static AUDIO_DIR: &str = "audio";

pub static AUDIO_SOURCES: [(&str, &str); 2] =
    [("Living Room", "living_room"), ("Office", "office")];

pub struct AppData {
    brain_addr: Addr<AudioBrain>,
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
        AudioBrainSession::new(data.brain_addr.clone()),
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

    if cfg!(not(debug_assertions)) {
        simple_logging::log_to_file("info.log", LevelFilter::Info)
            .expect("logger should not fail to initialize");
    } else {
        simple_logging::log_to_stderr(LevelFilter::Info);
    };

    let downloader = AudioDownloader;
    let downloader_addr = downloader.start();

    let queue_server = AudioBrain::new(downloader_addr);
    let server_addr = queue_server.start();

    let data = Data::new(AppData {
        brain_addr: server_addr,
    });

    HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allow_any_header();

        App::new()
            .app_data(data.clone())
            .wrap(cors)
            .service(get_con_to_queue)
            .service(receive_node_cmd)
            .service(get_node_stream)
    })
    .bind((addr, 50051))?
    .run()
    .await
}
