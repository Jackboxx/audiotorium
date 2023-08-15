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

    let data = Data::new(AppData {
        queue_server_addr: server_addr,
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
    })
    .bind((addr, 50051))?
    .run()
    .await
}
