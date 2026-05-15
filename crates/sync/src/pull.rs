use crate::cursor;
use crate::decrypt;
use crate::diff;
use crate::error::SyncError;
use crate::thumbnails;
use base64::Engine;
use crypto::Key256;
use local_db::LocalDb;
use local_db::collections;
use local_db::files;
use std::sync::Arc;
use thumbnail::cache::ThumbnailCache;
use tokio::sync::RwLock;
use tracing::{debug, info};
use types::collection::Collection;
use zoo_client::ZooClient;

pub struct SyncEngine {
    pub zoo_client: Arc<ZooClient>,
    pub local_db: Arc<LocalDb>,
    pub thumbnail_cache: Arc<ThumbnailCache>,
    pub master_key: Arc<RwLock<Option<Key256>>>,
}

pub async fn sync_all(engine: &SyncEngine) -> Result<(), SyncError> {
    info!("starting sync_all");

    sync_collections(engine).await?;
    sync_files(engine).await?;
    sync_trash(engine).await?;
    thumbnails::queue_new_files(&engine.local_db, &engine.thumbnail_cache).await?;

    info!("sync_all completed");
    Ok(())
}

async fn sync_collections(engine: &SyncEngine) -> Result<(), SyncError> {
    let since =
        cursor::read_cursor(&engine.local_db, "collections_since").unwrap_or(Some(0)).unwrap_or(0);

    let mut current_since = since;

    loop {
        let page = diff::fetch_collection_page(&engine.zoo_client, current_since).await?;

        if page.collections.is_empty() {
            break;
        }

        let master_key = engine.master_key.read().await;
        let mk = master_key
            .as_ref()
            .ok_or(SyncError::NetworkError(zoo_client::ZooError::NotAuthenticated))?;

        for coll in &page.collections {
            let encrypted_name_bytes = base64::prelude::BASE64_STANDARD
                .decode(&coll.encrypted_name)
                .map_err(|e| SyncError::ParseError(format!("base64 decode encrypted_name: {}", e)))?;

            let name_nonce_bytes = base64::prelude::BASE64_STANDARD
                .decode(&coll.name_decryption_nonce)
                .map_err(|e| SyncError::ParseError(format!("base64 decode name_nonce: {}", e)))?;

            let mut nonce_arr = [0u8; 24];
            nonce_arr.copy_from_slice(&name_nonce_bytes);

            let decrypted_name = crypto::aead::secretbox::secretbox_decrypt(
                &types::crypto::Nonce24::new(nonce_arr),
                &encrypted_name_bytes,
                mk,
            )
            .map_err(|e| {
                SyncError::ParseError(format!("decrypt collection name: {}", e))
            })?;

            let name = String::from_utf8(decrypted_name).map_err(|e| {
                SyncError::ParseError(format!("collection name not utf8: {}", e))
            })?;

            let record = Collection {
                id: coll.id.clone(),
                name,
                encrypted_key: coll.encrypted_key.clone(),
                key_nonce: coll.key_decryption_nonce.clone(),
                updation_time: coll.updation_time,
                created_at: 0,
                archived_at: None,
            };

            collections::upsert_collection(&engine.local_db.conn, &record)
                .map_err(SyncError::DbError)?;

            current_since = coll.updation_time;
        }

        cursor::write_cursor(
            &engine.local_db,
            "collections_since",
            page.latest_updated_at,
        )
        .map_err(|e| SyncError::CursorError(e.to_string()))?;

        if !page.has_more {
            break;
        }
    }

    debug!("collections sync completed, cursor={}", current_since);
    Ok(())
}

async fn sync_files(engine: &SyncEngine) -> Result<(), SyncError> {
    let collections_list =
        collections::list_collections(&engine.local_db.conn).map_err(SyncError::DbError)?;

    for collection in &collections_list {
        let since = cursor::read_cursor(
            &engine.local_db,
            &format!("collection:{}:since", collection.id),
        )
        .unwrap_or(Some(0))
        .unwrap_or(0);

        let mut current_since = since;

        loop {
            let page =
                diff::fetch_file_page(&engine.zoo_client, &collection.id, current_since).await?;

            if page.updated_files.is_empty() && page.deleted_file_ids.is_empty() {
                break;
            }

            let decrypted = decrypt::batch_decrypt_files(&page.updated_files, &Key256::new([0u8; 32]))?;

            if !decrypted.is_empty() {
                files::upsert_files(&engine.local_db.conn, &decrypted)
                    .map_err(SyncError::DbError)?;
            }

            if !page.deleted_file_ids.is_empty() {
                files::archive_files(&engine.local_db.conn, &page.deleted_file_ids)
                    .map_err(SyncError::DbError)?;
            }

            current_since = page.latest_updated_at;

            cursor::write_cursor(
                &engine.local_db,
                &format!("collection:{}:since", collection.id),
                page.latest_updated_at,
            )
            .map_err(|e| SyncError::CursorError(e.to_string()))?;

            if !page.has_more {
                break;
            }
        }

        debug!(
            "files sync completed for collection {}, cursor={}",
            collection.id, current_since
        );
    }

    Ok(())
}

async fn sync_trash(engine: &SyncEngine) -> Result<(), SyncError> {
    let since =
        cursor::read_cursor(&engine.local_db, "trash_since").unwrap_or(Some(0)).unwrap_or(0);

    let mut current_since = since;

    loop {
        let page = diff::fetch_trash_page(&engine.zoo_client, current_since).await?;

        if page.deleted_files.is_empty() {
            break;
        }

        let file_ids: Vec<i64> = page.deleted_files.iter().map(|f| f.file_id).collect();

        if !file_ids.is_empty() {
            files::archive_files(&engine.local_db.conn, &file_ids)
                .map_err(SyncError::DbError)?;
        }

        for deleted in &page.deleted_files {
            current_since = deleted.updation_time;
        }

        cursor::write_cursor(
            &engine.local_db,
            "trash_since",
            page.latest_updated_at,
        )
        .map_err(|e| SyncError::CursorError(e.to_string()))?;

        if !page.has_more {
            break;
        }
    }

    debug!("trash sync completed, cursor={}", current_since);
    Ok(())
}
