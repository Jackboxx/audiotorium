#![allow(dead_code)]

use std::fs::File;
use std::io::BufReader;
use std::process::Command;

use cpal::traits::{DeviceTrait, HostTrait};
use home_audio_manager::rpc_source_service_server::{RpcSourceService, RpcSourceServiceServer};
use home_audio_manager::{Empty, GetSourcesResponse};
use rodio::Decoder;
use tonic::{transport::Server, Request, Response, Status};

pub mod home_audio_manager {
    tonic::include_proto!("home_audio_manager");
}

#[derive(Debug, Default)]
pub struct SourceService;

#[tonic::async_trait]
impl RpcSourceService for SourceService {
    async fn get_sources(
        &self,
        request: Request<Empty>,
    ) -> Result<Response<GetSourcesResponse>, Status> {
        println!("Got a request: {:?}", request);

        let host = cpal::default_host();
        let devices = host.output_devices().unwrap();

        let res = GetSourcesResponse {
            names: devices.flat_map(|dev| dev.name()).collect(),
        };

        Ok(Response::new(res))
    }
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

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let addr = "[::1]:50051".parse()?;
    let manager = SourceService::default();

    Server::builder()
        .add_service(RpcSourceServiceServer::new(manager))
        .serve(addr)
        .await?;

    Ok(())
}
