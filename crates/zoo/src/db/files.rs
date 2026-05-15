use chrono::Utc;
use sqlx::{PgPool, Row};
use uuid::Uuid;
use crate::db::models::FileRecord;
use crate::error::ZooError;

pub async fn insert_file_record(
    pool: &PgPool,
    user_id: Uuid,
    collection_id: &str,
    encrypted_key: &str,
    key_decryption_nonce: &str,
    file_decryption_header: &str,
    thumb_decryption_header: Option<&str>,
    encrypted_metadata: &str,
    encrypted_thumbnail: Option<&str>,
    thumbnail_size: Option<i32>,
    file_size: i64,
    mime_type: &str,
    content_hash: &str,
    object_key: &str,
) -> Result<i64, ZooError> {
    let row = sqlx::query(
        "INSERT INTO files (user_id, collection_id, encrypted_key, key_decryption_nonce,
         file_decryption_header, thumb_decryption_header, encrypted_metadata,
         encrypted_thumbnail, thumbnail_size, file_size, mime_type, content_hash, object_key)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
         RETURNING id",
    )
    .bind(user_id)
    .bind(collection_id)
    .bind(encrypted_key)
    .bind(key_decryption_nonce)
    .bind(file_decryption_header)
    .bind(thumb_decryption_header)
    .bind(encrypted_metadata)
    .bind(encrypted_thumbnail)
    .bind(thumbnail_size)
    .bind(file_size)
    .bind(mime_type)
    .bind(content_hash)
    .bind(object_key)
    .fetch_one(pool)
    .await?;
    Ok(row.get::<i64, _>("id"))
}

pub async fn get_file_for_download(
    pool: &PgPool,
    user_id: Uuid,
    file_id: i64,
) -> Result<Option<FileRecord>, ZooError> {
    let file = sqlx::query_as::<_, FileRecord>(
        "SELECT * FROM files WHERE id = $1 AND user_id = $2 AND archived_at IS NULL",
    )
    .bind(file_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    Ok(file)
}

pub async fn list_files_for_sync(
    pool: &PgPool,
    user_id: Uuid,
    since: Option<chrono::DateTime<Utc>>,
    limit: i64,
) -> Result<Vec<FileRecord>, ZooError> {
    let files = match since {
        Some(s) => sqlx::query_as::<_, FileRecord>(
            "SELECT * FROM files WHERE user_id = $1 AND updation_time > $2 AND archived_at IS NULL
             ORDER BY updation_time ASC, id ASC LIMIT $3",
        )
        .bind(user_id)
        .bind(s)
        .bind(limit)
        .fetch_all(pool)
        .await?,
        None => sqlx::query_as::<_, FileRecord>(
            "SELECT * FROM files WHERE user_id = $1 AND archived_at IS NULL
             ORDER BY updation_time ASC, id ASC LIMIT $2",
        )
        .bind(user_id)
        .bind(limit)
        .fetch_all(pool)
        .await?,
    };
    Ok(files)
}

pub async fn archive_file(
    pool: &PgPool,
    user_id: Uuid,
    file_id: i64,
) -> Result<(), ZooError> {
    sqlx::query(
        "UPDATE files SET archived_at = NOW(), updation_time = NOW()
         WHERE id = $1 AND user_id = $2 AND archived_at IS NULL",
    )
    .bind(file_id)
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(())
}
