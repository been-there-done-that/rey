use sqlx::PgPool;
use uuid::Uuid;
use crate::db::models::UploadPart;
use crate::error::ZooError;

pub async fn insert_parts_batch(
    pool: &PgPool,
    upload_id: Uuid,
    parts: &[(i16, i32, String)],
) -> Result<(), ZooError> {
    for (part_number, part_size, part_md5) in parts {
        sqlx::query!(
            "INSERT INTO upload_parts (upload_id, part_number, part_size, part_md5) VALUES ($1, $2, $3, $4)",
            upload_id,
            part_number,
            part_size,
            part_md5,
        )
        .execute(pool)
        .await?;
    }
    Ok(())
}

pub async fn mark_part_uploaded(
    pool: &PgPool,
    upload_id: Uuid,
    part_number: i16,
    etag: &str,
) -> Result<(), ZooError> {
    sqlx::query!(
        "UPDATE upload_parts SET status = 'uploaded', etag = $1, uploaded_at = NOW()
         WHERE upload_id = $2 AND part_number = $3",
        etag,
        upload_id,
        part_number
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_pending_parts(
    pool: &PgPool,
    upload_id: Uuid,
) -> Result<Vec<UploadPart>, ZooError> {
    let parts = sqlx::query_as!(
        UploadPart,
        "SELECT * FROM upload_parts WHERE upload_id = $1 AND status = 'pending' ORDER BY part_number",
        upload_id
    )
    .fetch_all(pool)
    .await?;
    Ok(parts)
}

pub async fn list_uploaded_parts(
    pool: &PgPool,
    upload_id: Uuid,
) -> Result<Vec<UploadPart>, ZooError> {
    let parts = sqlx::query_as!(
        UploadPart,
        "SELECT * FROM upload_parts WHERE upload_id = $1 AND status = 'uploaded' ORDER BY part_number",
        upload_id
    )
    .fetch_all(pool)
    .await?;
    Ok(parts)
}
