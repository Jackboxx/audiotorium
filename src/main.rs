use cpal::traits::{DeviceTrait, HostTrait};
use home_audio_manager::rpc_device_service_server::{RpcDeviceService, RpcDeviceServiceServer};
use home_audio_manager::{Empty, GetDevicesResponse};
use tonic::{transport::Server, Request, Response, Status};

pub mod home_audio_manager {
    tonic::include_proto!("home_audio_manager");
}

#[derive(Debug, Default)]
pub struct DeviceService;

#[tonic::async_trait]
impl RpcDeviceService for DeviceService {
    async fn get_devices(
        &self,
        request: Request<Empty>,
    ) -> Result<Response<GetDevicesResponse>, Status> {
        println!("Got a request: {:?}", request);

        let host = cpal::default_host();
        let devices = host.output_devices().unwrap();

        let res = GetDevicesResponse {
            names: devices.flat_map(|dev| dev.name()).collect(),
        };

        Ok(Response::new(res))
    }
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let addr = "[::1]:50051".parse()?;
    let manager = DeviceService::default();

    Server::builder()
        .add_service(RpcDeviceServiceServer::new(manager))
        .serve(addr)
        .await?;

    Ok(())
}
