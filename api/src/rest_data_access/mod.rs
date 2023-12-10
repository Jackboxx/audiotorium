use std::sync::Arc;

use actix_web::{get, web, HttpResponse};
use serde::Serialize;

use crate::{
    audio_playback::audio_item::AudioMetadata,
    database::{
        fetch_data::{
            get_all_audio_metadata_from_db, get_all_playlist_metadata_from_db,
            get_playlist_items_from_db,
        },
        PlaylistMetadata,
    },
};

#[derive(Debug, Serialize)]
struct StoredAudioData {
    uid: Arc<str>,
    metadata: AudioMetadata,
}

#[derive(Debug, Serialize)]
struct StoredPlaylistData {
    uid: Arc<str>,
    metadata: PlaylistMetadata,
}

#[get("/data/playlists")]
pub async fn get_playlists() -> HttpResponse {
    match get_all_playlist_metadata_from_db().await {
        Ok(items) => {
            let result: Vec<StoredPlaylistData> = items
                .into_iter()
                .map(|(uid, metadata)| StoredPlaylistData {
                    uid: uid.0,
                    metadata,
                })
                .collect();

            HttpResponse::Ok().body(
                serde_json::to_string(&result).unwrap_or("oops something went wrong".to_owned()),
            )
        }
        Err(err) => {
            log::error!("{err:?}");
            HttpResponse::InternalServerError()
                .body(serde_json::to_string(&err).unwrap_or("oops something went wrong".to_owned()))
        }
    }
}

#[get("/data/audio")]
pub async fn get_audio() -> HttpResponse {
    match get_all_audio_metadata_from_db().await {
        Ok(items) => {
            let result: Vec<StoredAudioData> = items
                .into_iter()
                .map(|(uid, metadata)| StoredAudioData {
                    uid: uid.0,
                    metadata,
                })
                .collect();

            HttpResponse::Ok().body(
                serde_json::to_string(&result).unwrap_or("oops something went wrong".to_owned()),
            )
        }
        Err(err) => {
            log::error!("{err:?}");
            HttpResponse::InternalServerError()
                .body(serde_json::to_string(&err).unwrap_or("oops something went wrong".to_owned()))
        }
    }
}

#[get("/data/playlists/{playlist_uid}")]
pub async fn get_audio_in_playlist(playlist_uid: web::Path<Arc<str>>) -> HttpResponse {
    match get_playlist_items_from_db(playlist_uid.as_ref()).await {
        Ok(items) => {
            let result: Vec<StoredAudioData> = items
                .into_iter()
                .map(|(uid, metadata)| StoredAudioData {
                    uid: uid.0,
                    metadata,
                })
                .collect();

            HttpResponse::Ok().body(
                serde_json::to_string(&result).unwrap_or("oops something went wrong".to_owned()),
            )
        }
        Err(err) => {
            log::error!("{err:?}");
            HttpResponse::InternalServerError()
                .body(serde_json::to_string(&err).unwrap_or("oops something went wrong".to_owned()))
        }
    }
}
