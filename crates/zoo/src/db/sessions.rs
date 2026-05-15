use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;
use crate::db::models::Session;
use crate::error::ZooError;

pub async fn create_session(
    pool: &PgPool,
    user_id: Uuid,
    token_hash: &str,
    expires_at: chrono::DateTime<Utc>,
) -> Result<Uuid, ZooError> {
    let row = sqlx::query!(
        "INSERT INTO sessions (user_id, token_hash, expires_at) VALUES ($1, $2, $3) RETURNING id",
        user_id,
        token_hash,
        expires_at,
    )
    .fetch_one(pool)
    .await?;
    Ok(row.id)
}

pub async fn lookup_session(pool: &PgPool, token_hash: &str) -> Result<Option<Session>, ZooError> {
    let session = sqlx::query_as!(
        Session,
        "SELECT * FROM sessions WHERE token_hash = $1 AND expires_at > NOW()",
        token_hash
    )
    .fetch_optional(pool)
    .await?;
    Ok(session)
}

pub async fn revoke_session(pool: &PgPool, token_hash: &str) -> Result<(), ZooError> {
    sqlx::query!("DELETE FROM sessions WHERE token_hash = $1", token_hash)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn revoke_user_sessions(pool: &PgPool, user_id: Uuid) -> Result<(), ZooError> {
    sqlx::query!("DELETE FROM sessions WHERE user_id = $1", user_id)
        .execute(pool)
        .await?;
    Ok(())
}
