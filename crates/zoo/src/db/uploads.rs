use chrono::Utc;
use sqlx::{PgPool, Row};
use uuid::Uuid;
use crate::db::models::Upload;
use crate::error::ZooError;

pub async fn create_upload(
    pool: &PgPool,
    user_id: Uuid,
    device_id: Uuid,
    file_hash: &str,
    file_size: i64,
    mime_type: Option<&str>,
    part_size: i32,
    part_count: i16,
    expires_at: chrono::DateTime<Utc>,
) -> Result<Uuid, ZooError> {
    let row = sqlx::query(
        "INSERT INTO uploads (user_id, device_id, file_hash, file_size, mime_type, part_size, part_count, expires_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8) RETURNING id",
    )
    .bind(user_id)
    .bind(device_id)
    .bind(file_hash)
    .bind(file_size)
    .bind(mime_type)
    .bind(part_size)
    .bind(part_count)
    .bind(expires_at)
    .fetch_one(pool)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
            ZooError::UploadAlreadyExists
        }
        e => ZooError::Database(e),
    })?;
    Ok(row.get::<Uuid, _>("id"))
}

pub async fn get_upload(pool: &PgPool, id: Uuid) -> Result<Option<Upload>, ZooError> {
    let upload = sqlx::query_as::<_, Upload>("SELECT * FROM uploads WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(upload)
}

pub async fn patch_upload_status(
    pool: &PgPool,
    id: Uuid,
    status: &str,
) -> Result<(), ZooError> {
    sqlx::query(
        "UPDATE uploads SET status = $1 WHERE id = $2",
    )
    .bind(status)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn update_bitmask(
    pool: &PgPool,
    id: Uuid,
    bitmask: &[u8],
) -> Result<(), ZooError> {
    sqlx::query(
        "UPDATE uploads SET parts_bitmask = $1 WHERE id = $2",
    )
    .bind(bitmask)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn update_heartbeat(pool: &PgPool, id: Uuid) -> Result<(), ZooError> {
    sqlx::query(
        "UPDATE uploads SET last_heartbeat_at = NOW() WHERE id = $1",
    )
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn update_s3_info(
    pool: &PgPool,
    id: Uuid,
    upload_id_s3: &str,
    complete_url: &str,
    urls_expire_at: chrono::DateTime<Utc>,
) -> Result<(), ZooError> {
    sqlx::query(
        "UPDATE uploads SET upload_id_s3 = $1, complete_url = $2, urls_expire_at = $3 WHERE id = $4",
    )
    .bind(upload_id_s3)
    .bind(complete_url)
    .bind(urls_expire_at)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_uploads_by_status(
    pool: &PgPool,
    status: &str,
    limit: i64,
) -> Result<Vec<Upload>, ZooError> {
    let uploads = sqlx::query_as::<_, Upload>(
        "SELECT * FROM uploads WHERE status = $1 ORDER BY created_at ASC LIMIT $2",
    )
    .bind(status)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(uploads)
}

pub async fn list_stalled_uploads(
    pool: &PgPool,
    stall_threshold: chrono::DateTime<Utc>,
) -> Result<Vec<Upload>, ZooError> {
    let uploads = sqlx::query_as::<_, Upload>(
        "SELECT * FROM uploads WHERE status = 'uploading' AND last_heartbeat_at < $1 AND stalled_at IS NULL
         ORDER BY last_heartbeat_at ASC FOR UPDATE SKIP LOCKED",
    )
    .bind(stall_threshold)
    .fetch_all(pool)
    .await?;
    Ok(uploads)
}

pub async fn mark_stalled(pool: &PgPool, id: Uuid) -> Result<(), ZooError> {
    sqlx::query(
        "UPDATE uploads SET status = 'stalled', stalled_at = NOW() WHERE id = $1",
    )
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_expired_uploads(
    pool: &PgPool,
    now: chrono::DateTime<Utc>,
) -> Result<Vec<Upload>, ZooError> {
    let uploads = sqlx::query_as::<_, Upload>(
        "SELECT * FROM uploads WHERE expires_at < $1 AND status NOT IN ('done', 'failed')
         ORDER BY created_at ASC LIMIT 100 FOR UPDATE SKIP LOCKED",
    )
    .bind(now)
    .fetch_all(pool)
    .await?;
    Ok(uploads)
}
