use crate::db::models::User;
use crate::error::ZooError;
use sqlx::PgPool;
use uuid::Uuid;

#[allow(clippy::too_many_arguments)]
pub async fn register_user(
    pool: &PgPool,
    email: &str,
    verify_key_hash: &str,
    encrypted_master_key: &str,
    key_nonce: &str,
    kek_salt: &str,
    mem_limit: i32,
    ops_limit: i32,
    public_key: &str,
    encrypted_secret_key: &str,
    secret_key_nonce: &str,
    encrypted_recovery_key: &str,
    recovery_key_nonce: &str,
) -> Result<Uuid, ZooError> {
    let id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO users (email, verify_key_hash, encrypted_master_key, key_nonce, kek_salt,
         mem_limit, ops_limit, public_key, encrypted_secret_key, secret_key_nonce,
         encrypted_recovery_key, recovery_key_nonce)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
         RETURNING id",
    )
    .bind(email)
    .bind(verify_key_hash)
    .bind(encrypted_master_key)
    .bind(key_nonce)
    .bind(kek_salt)
    .bind(mem_limit)
    .bind(ops_limit)
    .bind(public_key)
    .bind(encrypted_secret_key)
    .bind(secret_key_nonce)
    .bind(encrypted_recovery_key)
    .bind(recovery_key_nonce)
    .fetch_one(pool)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
            ZooError::Validation("email already exists".to_string())
        }
        e => ZooError::Database(e),
    })?;

    Ok(id)
}

pub async fn find_user_by_email(pool: &PgPool, email: &str) -> Result<Option<User>, ZooError> {
    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE email = $1")
        .bind(email)
        .fetch_optional(pool)
        .await?;
    Ok(user)
}

pub async fn get_user_by_id(pool: &PgPool, id: Uuid) -> Result<Option<User>, ZooError> {
    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(user)
}
