use std::env;
use std::process::Command;

use actix::{Actor, Addr};
use actix_cors::Cors;
use actix_web::web::{self, Data};
use actix_web::{get, App, HttpRequest, HttpServer, Responder};
use actix_web_actors::ws;

use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use server::QueueServer;

use crate::session::QueueSession;

mod audio;
mod server;
mod session;

static AUDIO_DIR: &str = "audio";

pub struct AppData {
    queue_server_addr: Addr<QueueServer>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorResponse {
    error: String,
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
    ws::start(
        QueueSession::new(data.queue_server_addr.clone()),
        &req,
        stream,
    )
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env::set_var("RUST_LOG", "info");
    env_logger::init();

    let queue_server = QueueServer::default();
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
    .bind(("127.0.0.1", 50051))?
    .run()
    .await
}
