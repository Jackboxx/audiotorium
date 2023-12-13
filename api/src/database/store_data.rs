use crate::{
    db_pool,
    error::{AppError, AppErrorKind, IntoAppError},
};

use super::fetch_data::get_next_position_item_for_playlist;

pub async fn store_playlist_if_not_exists(uid: &str) -> Result<(), AppError> {
    let mut tx = db_pool().begin().await.into_app_err(
        "failed to start transaction",
        AppErrorKind::Database,
        &[],
    )?;

    sqlx::query!(
        "INSERT INTO audio_playlist
        (identifier) VALUES ($1)
        ON CONFLICT DO NOTHING",
        uid
    )
    .execute(&mut *tx)
    .await
    .into_app_err(
        "failed to create audio playlist",
        AppErrorKind::Database,
        &[&format!("UID: {uid}")],
    )?;

    tx.commit()
        .await
        .into_app_err("failed to commit transaction", AppErrorKind::Database, &[])
}

pub async fn store_playlist_item_relation_if_not_exists(
    playlist_uid: &str,
    audio_uid: &str,
) -> Result<(), AppError> {
    let position = get_next_position_item_for_playlist(playlist_uid).await?;

    let mut tx = db_pool().begin().await.into_app_err(
        "failed to start transaction",
        AppErrorKind::Database,
        &[],
    )?;

    sqlx::query!(
        "INSERT INTO audio_playlist_item
        (playlist_identifier, item_identifier, position) VALUES ($1, $2, $3)
        ON CONFLICT DO NOTHING",
        playlist_uid,
        audio_uid,
        position,
    )
    .execute(&mut *tx)
    .await
    .into_app_err(
        "failed to add audio to playlist",
        AppErrorKind::Database,
        &[
            &format!("PLAYLIST_UID: {playlist_uid}"),
            &format!("AUDIO_UID: {audio_uid}"),
        ],
    )?;

    tx.commit()
        .await
        .into_app_err("failed to commit transaction", AppErrorKind::Database, &[])
}
