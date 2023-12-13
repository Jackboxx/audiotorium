use std::sync::Arc;

use actix_web::{get, web, HttpResponse};
use serde::{Deserialize, Serialize};

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

#[derive(Deserialize)]
struct OffsetLimitParams {
    limit: Option<i64>,
    offset: Option<i64>,
}

#[get("/data/playlists")]
pub async fn get_playlists(
    web::Query(OffsetLimitParams { limit, offset }): web::Query<OffsetLimitParams>,
) -> HttpResponse {
    match get_all_playlist_metadata_from_db(limit, offset).await {
        Ok(items) => {
            let result: Vec<StoredPlaylistData> = items
                .iter()
                .map(|(uid, metadata)| StoredPlaylistData {
                    uid: Arc::clone(&uid.0),
                    metadata: metadata.clone(),
                })
                .collect();

            HttpResponse::Ok().body(
                serde_json::to_string(&result).unwrap_or("oops something went wrong".to_owned()),
            )
        }
        Err(err) => HttpResponse::InternalServerError()
            .body(serde_json::to_string(&err).unwrap_or("oops something went wrong".to_owned())),
    }
}

#[get("/data/audio")]
pub async fn get_audio(
    web::Query(OffsetLimitParams { limit, offset }): web::Query<OffsetLimitParams>,
) -> HttpResponse {
    match get_all_audio_metadata_from_db(limit, offset).await {
        Ok(items) => {
            let result: Vec<StoredAudioData> = items
                .iter()
                .map(|(uid, metadata)| StoredAudioData {
                    uid: Arc::clone(&uid.0),
                    metadata: metadata.clone(),
                })
                .collect();

            HttpResponse::Ok().body(
                serde_json::to_string(&result).unwrap_or("oops something went wrong".to_owned()),
            )
        }
        Err(err) => HttpResponse::InternalServerError()
            .body(serde_json::to_string(&err).unwrap_or("oops something went wrong".to_owned())),
    }
}

#[get("/data/playlists/{playlist_uid}")]
pub async fn get_audio_in_playlist(
    playlist_uid: web::Path<Arc<str>>,
    web::Query(OffsetLimitParams { limit, offset }): web::Query<OffsetLimitParams>,
) -> HttpResponse {
    match get_playlist_items_from_db(playlist_uid.as_ref(), limit, offset).await {
        Ok(items) => {
            let result: Vec<StoredAudioData> = items
                .iter()
                .map(|(uid, metadata)| StoredAudioData {
                    uid: Arc::clone(&uid.0),
                    metadata: metadata.clone(),
                })
                .collect();

            HttpResponse::Ok().body(
                serde_json::to_string(&result).unwrap_or("oops something went wrong".to_owned()),
            )
        }
        Err(err) => HttpResponse::InternalServerError()
            .body(serde_json::to_string(&err).unwrap_or("oops something went wrong".to_owned())),
    }
}
