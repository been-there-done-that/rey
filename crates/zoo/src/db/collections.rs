use crate::error::ZooError;
use crate::db::models::Collection;
use sqlx::{PgPool, Row};
use uuid::Uuid;

pub async fn create_collection(
    pool: &PgPool,
    user_id: Uuid,
    encrypted_name: &str,
    encrypted_key: &str,
    key_decryption_nonce: &str,
    encrypted_metadata: Option<&str>,
) -> Result<Uuid, ZooError> {
    let row = sqlx::query(
        r#"
        INSERT INTO collections (user_id, encrypted_name, encrypted_key, key_decryption_nonce, encrypted_metadata)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(encrypted_name)
    .bind(encrypted_key)
    .bind(key_decryption_nonce)
    .bind(encrypted_metadata)
    .fetch_one(pool)
    .await?;

    Ok(row.get("id"))
}

pub async fn list_collections(
    pool: &PgPool,
    user_id: Uuid,
    since_time: i64,
) -> Result<Vec<Collection>, ZooError> {
    let since = chrono::DateTime::from_timestamp_millis(since_time)
        .unwrap_or(chrono::DateTime::UNIX_EPOCH);

    let rows = sqlx::query_as::<_, Collection>(
        r#"
        SELECT id, user_id, encrypted_name, encrypted_key, key_decryption_nonce,
               encrypted_metadata, created_at, updation_time
        FROM collections
        WHERE user_id = $1 AND updation_time > $2
        ORDER BY updation_time ASC
        "#,
    )
    .bind(user_id)
    .bind(since)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

pub async fn update_collection(
    pool: &PgPool,
    user_id: Uuid,
    collection_id: Uuid,
    encrypted_name: Option<&str>,
    encrypted_metadata: Option<&str>,
) -> Result<(), ZooError> {
    let result = sqlx::query(
        r#"
        UPDATE collections
        SET encrypted_name = COALESCE($3, encrypted_name),
            encrypted_metadata = COALESCE($4, encrypted_metadata),
            updation_time = NOW()
        WHERE id = $1 AND user_id = $2
        "#,
    )
    .bind(collection_id)
    .bind(user_id)
    .bind(encrypted_name)
    .bind(encrypted_metadata)
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(ZooError::NotFound);
    }

    Ok(())
}

pub async fn delete_collection(
    pool: &PgPool,
    user_id: Uuid,
    collection_id: Uuid,
) -> Result<(), ZooError> {
    let result = sqlx::query(
        r#"
        DELETE FROM collections WHERE id = $1 AND user_id = $2
        "#,
    )
    .bind(collection_id)
    .bind(user_id)
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(ZooError::NotFound);
    }

    Ok(())
}
