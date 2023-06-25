#![allow(dead_code)]

use std::fs::File;
use std::io::BufReader;
use std::process::Command;

use actix_web::{get, App, HttpResponse, HttpServer, Responder};
use cpal::traits::{DeviceTrait, HostTrait};
use rodio::Decoder;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| App::new().service(get_sources))
        .bind(("127.0.0.1", 50051))?
        .run()
        .await
}

#[get("/sources")]
async fn get_sources() -> impl Responder {
    let host = cpal::default_host();
    let devices = host.output_devices().unwrap();

    HttpResponse::Ok().json(devices.flat_map(|dev| dev.name()).collect::<Vec<_>>())
}

fn get_audio_from_file(title: &str, url: &str) -> Result<Decoder<BufReader<File>>, anyhow::Error> {
    let path = format!("audio/{title}");
    let file = match File::open(&path) {
        Ok(file) => file,
        Err(err) => match err.kind() {
            std::io::ErrorKind::NotFound => {
                download_audio(url, &path)?;
                File::open(title)?
            }
            _ => return Err(err.into()),
        },
    };

    Ok(Decoder::new(BufReader::new(file))?)
}

fn download_audio(url: &str, download_location: &str) -> anyhow::Result<()> {
    Command::new("yt-dlp")
        .args(["-x", "--bestaudio", &format!("-o {download_location}"), url])
        .output()?;
    Ok(())
}
