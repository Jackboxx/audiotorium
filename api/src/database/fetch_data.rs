use std::sync::Arc;

use crate::{
    audio_playback::audio_item::AudioMetadata, db_pool, downloader::download_identifier::ItemUid,
    ErrorResponse, IntoErrResp, OptionArc,
};

use super::PlaylistMetadata;

struct AudioQueryResult {
    identifier: Arc<str>,
    name: Option<String>,
    author: Option<String>,
    duration: Option<i64>,
    cover_art_url: Option<String>,
}

struct PlaylistQueryResult {
    identifier: Arc<str>,
    name: OptionArc<str>,
    author: OptionArc<str>,
    cover_art_url: OptionArc<str>,
}

impl From<AudioQueryResult> for (ItemUid<Arc<str>>, AudioMetadata) {
    fn from(value: AudioQueryResult) -> Self {
        (
            ItemUid(value.identifier),
            AudioMetadata {
                name: value.name,
                author: value.author,
                duration: value.duration,
                cover_art_url: value.cover_art_url,
            },
        )
    }
}

impl From<PlaylistQueryResult> for (ItemUid<Arc<str>>, PlaylistMetadata) {
    fn from(value: PlaylistQueryResult) -> Self {
        (
            ItemUid(value.identifier),
            PlaylistMetadata {
                name: value.name.into(),
                author: value.author.into(),
                cover_art_url: value.cover_art_url.into(),
            },
        )
    }
}

pub async fn get_audio_metadata_from_db(uid: &str) -> Result<Option<AudioMetadata>, ErrorResponse> {
    sqlx::query_as!(
        AudioMetadata,
        "SELECT name, author, duration, cover_art_url FROM audio_metadata where identifier = $1",
        uid
    )
    .fetch_optional(db_pool())
    .await
    .into_err_resp("")
}

pub async fn get_all_audio_metadata_from_db(
) -> Result<Vec<(ItemUid<Arc<str>>, AudioMetadata)>, ErrorResponse> {
    sqlx::query_as!(
        AudioQueryResult,
        "SELECT identifier, name, author, duration, cover_art_url FROM audio_metadata",
    )
    .fetch_all(db_pool())
    .await
    .map(|vec| vec.into_iter().map(Into::into).collect())
    .into_err_resp("")
}

pub async fn get_all_playlist_metadata_from_db(
) -> Result<Vec<(ItemUid<Arc<str>>, PlaylistMetadata)>, ErrorResponse> {
    sqlx::query_as!(
        PlaylistQueryResult,
        "SELECT identifier, name, author, cover_art_url FROM audio_playlist",
    )
    .fetch_all(db_pool())
    .await
    .map(|vec| vec.into_iter().map(Into::into).collect())
    .into_err_resp("")
}

pub async fn get_playlist_items_from_db(
    playlist_uid: &str,
) -> Result<Vec<(ItemUid<Arc<str>>, AudioMetadata)>, ErrorResponse> {
    sqlx::query_as!(
        AudioQueryResult,
        "SELECT audio.identifier, audio.name, audio.author, audio.duration, audio.cover_art_url 
            FROM audio_metadata audio
        INNER JOIN audio_playlist_item items 
            ON items.playlist_identifier = $1",
        playlist_uid
    )
    .fetch_all(db_pool())
    .await
    .map(|vec| vec.into_iter().map(Into::into).collect())
    .into_err_resp("")
}
