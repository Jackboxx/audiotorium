use std::{env, fs};

use actix::Actor;
use actix_rt::Arbiter;
use audio_manager_api::brain::brain_server::AudioBrain;
use audio_manager_api::commands::node_commands::receive_node_cmd;
use audio_manager_api::downloader::actor::AudioDownloader;
use audio_manager_api::downloader::AUDIO_DIR;
use audio_manager_api::rest_data_access::{get_audio, get_audio_in_playlist, get_playlists};
use audio_manager_api::streams::brain_streams::get_brain_stream;
use audio_manager_api::streams::node_streams::get_node_stream;
use audio_manager_api::{db_pool, AppData, POOL, YOUTUBE_API_KEY};
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

    clear_dev_db().await;

    HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allow_any_header();

        App::new()
            .app_data(data.clone())
            .wrap(cors)
            .service(get_brain_stream)
            .service(get_node_stream)
            .service(receive_node_cmd)
            .service(get_audio)
            .service(get_playlists)
            .service(get_audio_in_playlist)
    })
    .bind((addr, 50051))?
    .run()
    .await
}

async fn clear_dev_db() {
    let should_clear = env::args().any(|str| str == "-c");

    if should_clear && cfg!(debug_assertions) {
        println!(
            "
============================
||                        ||
||  REMOVING DEV DATABASE ||
||                        ||
============================"
        );

        sqlx::query!("DELETE FROM audio_metadata")
            .execute(db_pool())
            .await
            .unwrap();

        sqlx::query!("DELETE FROM audio_playlist")
            .execute(db_pool())
            .await
            .unwrap();

        fs::remove_dir_all(AUDIO_DIR).unwrap();
        fs::create_dir(AUDIO_DIR).unwrap();
    }
}
