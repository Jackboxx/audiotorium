use std::env;

use actix::Actor;
use actix_rt::Arbiter;
use audio_manager_api::brain::brain_server::AudioBrain;
use audio_manager_api::commands::node_commands::receive_node_cmd;
use audio_manager_api::downloader::AudioDownloader;
use audio_manager_api::streams::brain_streams::get_brain_stream;
use audio_manager_api::streams::node_streams::get_node_stream;
use audio_manager_api::{AppData, POOL, YOUTUBE_API_KEY};
use log::LevelFilter;

use actix_cors::Cors;
use actix_web::web::Data;
use actix_web::{App, HttpServer};
use sqlx::postgres::PgPoolOptions;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv::dotenv().ok();

    let addr;
    if cfg!(not(debug_assertions)) {
        addr = dotenv::var("API_ADDRESS_PROD")
            .expect("environment variable 'API_ADDRESS_PROD' should exist for production builds");

        simple_logging::log_to_file("info.log", LevelFilter::Info)
            .expect("logger should not fail to initialize");
    } else {
        addr = dotenv::var("API_ADDRESS_DEV")
            .expect("environment variable 'API_ADDRESS_DEV' should exist for debug builds");

        simple_logging::log_to_stderr(LevelFilter::Info);
    };

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(env!("DATABASE_URL"))
        .await
        .unwrap();

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("all migrations should be valid");

    let youtube_api_key =
        dotenv::var("YOUTUE_API_KEY").expect("environment variable 'YOUTUBE_API_KEY' should exist");

    POOL.set(pool).expect("should never fail");
    YOUTUBE_API_KEY
        .set(youtube_api_key)
        .expect("should never fail");

    let download_arbiter = Arbiter::new();

    let downloader = AudioDownloader::new(download_arbiter);
    let downloader_addr = downloader.start();

    let queue_server = AudioBrain::new(downloader_addr);
    let server_addr = queue_server.start();

    let data = Data::new(AppData::new(server_addr));

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
