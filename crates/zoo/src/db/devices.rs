use crate::db::models::Device;
use crate::error::ZooError;
use sqlx::{PgPool, Row};
use uuid::Uuid;

pub async fn register_device(
    pool: &PgPool,
    user_id: Uuid,
    name: &str,
    platform: &str,
    sse_token: &str,
    push_token: Option<&str>,
    stall_timeout_seconds: i32,
) -> Result<Uuid, ZooError> {
    let row = sqlx::query(
        "INSERT INTO devices (user_id, name, platform, sse_token, push_token, stall_timeout_seconds)
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
    )
    .bind(user_id)
    .bind(name)
    .bind(platform)
    .bind(sse_token)
    .bind(push_token)
    .bind(stall_timeout_seconds)
    .fetch_one(pool)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
            ZooError::DeviceNameTaken
        }
        e => ZooError::Database(e),
    })?;
    Ok(row.get::<Uuid, _>("id"))
}

pub async fn lookup_by_sse_token(
    pool: &PgPool,
    sse_token: &str,
) -> Result<Option<Device>, ZooError> {
    let device = sqlx::query_as::<_, Device>(
        "SELECT * FROM devices WHERE sse_token = $1 AND is_active = TRUE",
    )
    .bind(sse_token)
    .fetch_optional(pool)
    .await?;
    Ok(device)
}

pub async fn lookup_device_by_id(
    pool: &PgPool,
    device_id: Uuid,
) -> Result<Option<Device>, ZooError> {
    let device = sqlx::query_as::<_, Device>(
        "SELECT * FROM devices WHERE id = $1 AND is_active = TRUE",
    )
    .bind(device_id)
    .fetch_optional(pool)
    .await?;
    Ok(device)
}

pub async fn tombstone_device(pool: &PgPool, device_id: Uuid) -> Result<(), ZooError> {
    sqlx::query("UPDATE devices SET is_active = FALSE WHERE id = $1")
        .bind(device_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn get_device_stall_timeout(pool: &PgPool, device_id: Uuid) -> Result<i32, ZooError> {
    let row = sqlx::query("SELECT stall_timeout_seconds FROM devices WHERE id = $1")
        .bind(device_id)
        .fetch_optional(pool)
        .await?;
    Ok(row
        .map(|r| r.get::<i32, _>("stall_timeout_seconds"))
        .unwrap_or(90))
}

pub async fn update_last_seen(pool: &PgPool, device_id: Uuid) -> Result<(), ZooError> {
    sqlx::query("UPDATE devices SET last_seen_at = NOW() WHERE id = $1")
        .bind(device_id)
        .execute(pool)
        .await?;
    Ok(())
}
