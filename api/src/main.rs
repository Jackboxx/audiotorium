use std::env;
use std::io::BufReader;
use std::path::Path;
use std::process::Command;
use std::sync::Mutex;
use std::{fs::File, path::PathBuf};

use actix_cors::Cors;
use actix_web::web::{Data, Query};
use actix_web::{get, post, App, HttpResponse, HttpServer, Responder};
use cpal::traits::{DeviceTrait, HostTrait};
use rodio::Decoder;
use serde::{Deserialize, Serialize};

static AUDIO_DIR: &str = "audio";

struct AppState {
    queue: Vec<PathBuf>,
}

#[derive(Deserialize)]
struct AddQueueParams {
    pub title: String,
    pub url: String,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

#[allow(dead_code)]
fn get_audio_from_file(title: &str) -> anyhow::Result<Decoder<BufReader<File>>> {
    let path = Path::new(AUDIO_DIR).join(title);
    let file = File::open(&path)?;
    Ok(Decoder::new(BufReader::new(file))?)
}

fn download_audio(url: &str, download_location: &str) -> anyhow::Result<()> {
    Command::new("yt-dlp")
        .args(["-x", "--bestaudio", &format!("-o {download_location}"), url])
        .output()?;
    Ok(())
}

#[get("/sources")]
async fn get_sources() -> impl Responder {
    let host = cpal::default_host();
    let devices = host.output_devices().unwrap();

    HttpResponse::Ok().json(devices.flat_map(|dev| dev.name()).collect::<Vec<_>>())
}

#[post("/queue")]
async fn add_to_queue(
    data: Data<Mutex<AppState>>,
    params: Query<AddQueueParams>,
) -> impl Responder {
    let Ok(mut data) = data.lock() else { return HttpResponse::InternalServerError().json(ErrorResponse { error: "failed to acquire lock for application state".to_owned()})};

    let AddQueueParams { title, url } = params.0;
    let path = Path::new(AUDIO_DIR).join(&title);

    if !path.exists() {
        let Some(str_path) = path.to_str() else { return HttpResponse::InternalServerError().json(ErrorResponse { error: format!( "failed to construct valid path with title: {title}" )});};
        match download_audio(&url, str_path) {
            Err(err) => {
                return HttpResponse::InternalServerError().json(ErrorResponse {
                    error: format!("{err}"),
                })
            }
            _ => {}
        }
    }

    data.queue.push(path);
    HttpResponse::Ok().json(data.queue.clone())
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env::set_var("RUST_LOG", "actix_web=debug,actix_server=info");
    env_logger::init();

    HttpServer::new(|| {
        let cors = Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allow_any_header();

        App::new()
            .wrap(cors)
            .app_data(Data::new(Mutex::new(AppState { queue: Vec::new() })))
            .service(get_sources)
            .service(add_to_queue)
    })
    .bind(("127.0.0.1", 50051))?
    .run()
    .await
}
