use actix::Actor;
use audio_manager_api::brain::brain_server::AudioBrain;
use audio_manager_api::commands::node_commands::receive_node_cmd;
use audio_manager_api::downloader::AudioDownloader;
use audio_manager_api::streams::brain_streams::get_brain_stream;
use audio_manager_api::streams::node_streams::get_node_stream;
use audio_manager_api::AppData;
use log::LevelFilter;

use actix_cors::Cors;
use actix_web::web::Data;
use actix_web::{App, HttpServer};

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

    let downloader = AudioDownloader::default();
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
            .service(get_brain_stream)
            .service(receive_node_cmd)
            .service(get_node_stream)
    })
    .bind((addr, 50051))?
    .run()
    .await
}
