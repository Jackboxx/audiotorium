use crate::{db_pool, ErrorResponse, IntoErrResp};

pub async fn store_playlist_if_not_exists(uid: &str) -> Result<(), ErrorResponse> {
    let mut tx = db_pool().begin().await.into_err_resp("")?;

    sqlx::query!(
        "INSERT INTO audio_playlist
        (identifier) VALUES ($1)
        ON CONFLICT DO NOTHING",
        uid
    )
    .execute(&mut *tx)
    .await
    .into_err_resp("")?;

    tx.commit().await.into_err_resp("")
}

pub async fn store_playlist_item_relation_if_not_exists(
    playlist_uid: &str,
    audio_uid: &str,
) -> Result<(), ErrorResponse> {
    let mut tx = db_pool().begin().await.into_err_resp("")?;

    sqlx::query!(
        "INSERT INTO audio_playlist_item
        (playlist_identifier, item_identifier) VALUES ($1, $2)
        ON CONFLICT DO NOTHING",
        playlist_uid,
        audio_uid
    )
    .execute(&mut *tx)
    .await
    .into_err_resp("")?;

    tx.commit().await.into_err_resp("")
}
