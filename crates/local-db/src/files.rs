use crate::error::LocalDbError;
use common::time::now_ms;
use rusqlite::{params, Connection, OptionalExtension};
use types::file::FileRecord;

pub fn upsert_files(conn: &Connection, files: &[FileRecord]) -> Result<(), LocalDbError> {
    let tx = conn.unchecked_transaction()?;
    {
        let mut stmt = tx.prepare(
            "INSERT INTO files (id, collection_id, cipher, title, description, latitude, longitude,
             taken_at, file_size, mime_type, content_hash, encrypted_key, key_nonce,
             file_decryption_header, thumb_decryption_header, object_key, thumbnail_path,
             updation_time, created_at, archived_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)
             ON CONFLICT(id) DO UPDATE SET
                 collection_id = ?2, cipher = ?3, title = ?4, description = ?5,
                 latitude = ?6, longitude = ?7, taken_at = ?8, file_size = ?9,
                 mime_type = ?10, content_hash = ?11, encrypted_key = ?12, key_nonce = ?13,
                 file_decryption_header = ?14, thumb_decryption_header = ?15,
                 object_key = ?16, thumbnail_path = ?17, updation_time = ?18,
                 created_at = ?19, archived_at = ?20"
        )?;
        for f in files {
            stmt.execute(params![
                f.id,
                &f.collection_id,
                &f.cipher,
                &f.title,
                &f.description,
                f.latitude,
                f.longitude,
                f.taken_at,
                f.file_size,
                &f.mime_type,
                &f.content_hash,
                &f.encrypted_key,
                &f.key_nonce,
                &f.file_decryption_header,
                &f.thumb_decryption_header,
                &f.object_key,
                &f.thumbnail_path,
                f.updation_time,
                f.created_at,
                f.archived_at,
            ])?;
        }
    }
    tx.commit()?;
    Ok(())
}

pub fn archive_files(conn: &Connection, ids: &[i64]) -> Result<(), LocalDbError> {
    let now = now_ms();
    let tx = conn.unchecked_transaction()?;
    {
        let mut stmt = tx.prepare("UPDATE files SET archived_at = ?1 WHERE id = ?2")?;
        for id in ids {
            stmt.execute((now, id))?;
        }
    }
    tx.commit()?;
    Ok(())
}

pub fn row_to_file_record(row: &rusqlite::Row<'_>) -> Result<FileRecord, rusqlite::Error> {
    Ok(FileRecord {
        id: row.get(0)?,
        collection_id: row.get(1)?,
        cipher: row.get(2)?,
        title: row.get(3)?,
        description: row.get(4)?,
        latitude: row.get(5)?,
        longitude: row.get(6)?,
        taken_at: row.get(7)?,
        file_size: row.get(8)?,
        mime_type: row.get(9)?,
        content_hash: row.get(10)?,
        encrypted_key: row.get(11)?,
        key_nonce: row.get(12)?,
        file_decryption_header: row.get(13)?,
        thumb_decryption_header: row.get(14)?,
        object_key: row.get(15)?,
        thumbnail_path: row.get(16)?,
        updation_time: row.get(17)?,
        created_at: row.get(18)?,
        archived_at: row.get(19)?,
    })
}

pub fn list_files(conn: &Connection, collection_id: &str) -> Result<Vec<FileRecord>, LocalDbError> {
    let mut stmt = conn.prepare(
        "SELECT id, collection_id, cipher, title, description, latitude, longitude,
         taken_at, file_size, mime_type, content_hash, encrypted_key, key_nonce,
         file_decryption_header, thumb_decryption_header, object_key, thumbnail_path,
         updation_time, created_at, archived_at
         FROM files WHERE collection_id = ?1 AND archived_at IS NULL
         ORDER BY taken_at DESC",
    )?;
    let files = stmt
        .query_map([collection_id], row_to_file_record)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(files)
}

pub fn get_file(conn: &Connection, id: i64) -> Result<Option<FileRecord>, LocalDbError> {
    let mut stmt = conn.prepare(
        "SELECT id, collection_id, cipher, title, description, latitude, longitude,
         taken_at, file_size, mime_type, content_hash, encrypted_key, key_nonce,
         file_decryption_header, thumb_decryption_header, object_key, thumbnail_path,
         updation_time, created_at, archived_at
         FROM files WHERE id = ?1",
    )?;
    let file = stmt.query_row([id], row_to_file_record).optional()?;
    Ok(file)
}

pub fn list_files_without_thumbnail(conn: &Connection) -> Result<Vec<FileRecord>, LocalDbError> {
    let mut stmt = conn.prepare(
        "SELECT id, collection_id, cipher, title, description, latitude, longitude,
         taken_at, file_size, mime_type, content_hash, encrypted_key, key_nonce,
         file_decryption_header, thumb_decryption_header, object_key, thumbnail_path,
         updation_time, created_at, archived_at
         FROM files WHERE thumbnail_path IS NULL AND archived_at IS NULL",
    )?;
    let files = stmt
        .query_map([], row_to_file_record)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connection::LocalDb;

    fn test_db() -> (LocalDb, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let db = LocalDb::open_test(&dir.path().join("test.db")).unwrap();
        (db, dir)
    }

    fn make_file(id: i64) -> FileRecord {
        FileRecord {
            id,
            collection_id: "col-1".to_string(),
            cipher: "xchacha20-poly1305".to_string(),
            title: Some(format!("photo_{id}.jpg")),
            description: None,
            latitude: None,
            longitude: None,
            taken_at: Some(1700000000000 + id),
            file_size: 1024,
            mime_type: "image/jpeg".to_string(),
            content_hash: format!("hash_{id}"),
            encrypted_key: "ek".to_string(),
            key_nonce: "kn".to_string(),
            file_decryption_header: "fdh".to_string(),
            thumb_decryption_header: None,
            object_key: format!("obj/{id}"),
            thumbnail_path: None,
            updation_time: 1700000000000,
            created_at: 1700000000000,
            archived_at: None,
        }
    }

    #[test]
    fn test_upsert_and_list_files() {
        let (db, _dir) = test_db();
        let files = vec![make_file(1), make_file(2)];
        upsert_files(&db.conn, &files).unwrap();
        let listed = list_files(&db.conn, "col-1").unwrap();
        assert_eq!(listed.len(), 2);
    }

    #[test]
    fn test_get_file() {
        let (db, _dir) = test_db();
        upsert_files(&db.conn, &[make_file(1)]).unwrap();
        let file = get_file(&db.conn, 1).unwrap().unwrap();
        assert_eq!(file.title, Some("photo_1.jpg".to_string()));
    }

    #[test]
    fn test_archive_files() {
        let (db, _dir) = test_db();
        upsert_files(&db.conn, &[make_file(1), make_file(2)]).unwrap();
        archive_files(&db.conn, &[1]).unwrap();
        let listed = list_files(&db.conn, "col-1").unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, 2);
    }

    #[test]
    fn test_list_files_without_thumbnail() {
        let (db, _dir) = test_db();
        let mut f = make_file(1);
        f.thumbnail_path = Some("/tmp/thumb.jpg".to_string());
        upsert_files(&db.conn, &[f, make_file(2)]).unwrap();
        let no_thumb = list_files_without_thumbnail(&db.conn).unwrap();
        assert_eq!(no_thumb.len(), 1);
        assert_eq!(no_thumb[0].id, 2);
    }
}
