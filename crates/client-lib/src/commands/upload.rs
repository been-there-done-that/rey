use crate::commands::error::CommandError;
use crate::state::AppState;
use sha2::Digest;
use types::upload::UploadState;

pub async fn upload_file(
    state: tauri::State<'_, AppState>,
    file_path: String,
    collection_id: String,
) -> Result<i64, CommandError> {
    let bytes = std::fs::read(&file_path).map_err(CommandError::Io)?;

    let hash_bytes = sha2::Sha256::digest(&bytes);
    let file_hash = hex::encode(hash_bytes);

    let file_size = bytes.len() as i64;
    let mime_type = infer::get(&bytes)
        .map(|t| t.mime_type())
        .unwrap_or("application/octet-stream")
        .to_string();

    let part_size = 20 * 1024 * 1024;
    let part_count = ((file_size + part_size as i64 - 1) / part_size as i64) as u16;

    let mut part_md5s = Vec::new();
    for i in 0..part_count {
        let start = (i as usize) * part_size;
        let end = std::cmp::min(start + part_size, bytes.len());
        let part_bytes = &bytes[start..end];
        let md5 = md5::compute(part_bytes);
        part_md5s.push(format!("{:x}", md5));
    }

    let file_id = state
        .zoo_client
        .upload_file(
            &bytes,
            &file_hash,
            part_md5s,
            file_size,
            &mime_type,
            &collection_id,
        )
        .await?;

    Ok(file_id)
}

pub async fn cancel_upload(
    state: tauri::State<'_, AppState>,
    upload_id: String,
) -> Result<(), CommandError> {
    let upload_id = uuid::Uuid::parse_str(&upload_id)
        .map_err(|e| CommandError::Validation(format!("invalid upload_id: {}", e)))?;
    state.zoo_client.cancel_upload(upload_id).await?;
    Ok(())
}

pub async fn list_pending_uploads(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<UploadState>, CommandError> {
    let uploads = state.zoo_client.pending_uploads().await?;
    Ok(uploads)
}
