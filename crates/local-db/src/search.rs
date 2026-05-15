use crate::error::LocalDbError;
use crate::files::row_to_file_record;
use rusqlite::Connection;
use types::file::FileRecord;

pub fn search_text(
    conn: &Connection,
    query: &str,
    limit: usize,
) -> Result<Vec<FileRecord>, LocalDbError> {
    let mut stmt = conn.prepare(
        "SELECT f.id, f.collection_id, f.cipher, f.title, f.description, f.latitude, f.longitude,
         f.taken_at, f.file_size, f.mime_type, f.content_hash, f.encrypted_key, f.key_nonce,
         f.file_decryption_header, f.thumb_decryption_header, f.object_key, f.thumbnail_path,
         f.updation_time, f.created_at, f.archived_at
         FROM files f
         JOIN files_fts fts ON fts.rowid = f.id
         WHERE files_fts MATCH ?1
           AND f.archived_at IS NULL
         ORDER BY f.taken_at DESC
         LIMIT ?2",
    )?;
    let files = stmt
        .query_map((&query, limit), row_to_file_record)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(files)
}

pub fn search_by_date(
    conn: &Connection,
    start_ms: i64,
    end_ms: i64,
    limit: usize,
) -> Result<Vec<FileRecord>, LocalDbError> {
    let mut stmt = conn.prepare(
        "SELECT id, collection_id, cipher, title, description, latitude, longitude,
         taken_at, file_size, mime_type, content_hash, encrypted_key, key_nonce,
         file_decryption_header, thumb_decryption_header, object_key, thumbnail_path,
         updation_time, created_at, archived_at
         FROM files
         WHERE taken_at >= ?1 AND taken_at <= ?2 AND archived_at IS NULL
         ORDER BY taken_at DESC
         LIMIT ?3",
    )?;
    let files = stmt
        .query_map((start_ms, end_ms, limit), row_to_file_record)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(files)
}

pub fn search_by_location(
    conn: &Connection,
    lat_min: f64,
    lat_max: f64,
    lon_min: f64,
    lon_max: f64,
    limit: usize,
) -> Result<Vec<FileRecord>, LocalDbError> {
    let mut stmt = conn.prepare(
        "SELECT id, collection_id, cipher, title, description, latitude, longitude,
         taken_at, file_size, mime_type, content_hash, encrypted_key, key_nonce,
         file_decryption_header, thumb_decryption_header, object_key, thumbnail_path,
         updation_time, created_at, archived_at
         FROM files
         WHERE latitude >= ?1 AND latitude <= ?2
           AND longitude >= ?3 AND longitude <= ?4
           AND archived_at IS NULL
         ORDER BY taken_at DESC
         LIMIT ?5",
    )?;
    let files = stmt
        .query_map(
            (lat_min, lat_max, lon_min, lon_max, limit),
            row_to_file_record,
        )?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(files)
}

pub fn rebuild_fts_index(conn: &Connection) -> Result<(), LocalDbError> {
    conn.execute("INSERT INTO files_fts(files_fts) VALUES('rebuild')", [])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connection::LocalDb;
    use crate::files::upsert_files;

    fn test_db() -> (LocalDb, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let db = LocalDb::open_test(&dir.path().join("test.db")).unwrap();
        (db, dir)
    }

    fn make_file(
        id: i64,
        title: &str,
        desc: &str,
        lat: Option<f64>,
        lon: Option<f64>,
        taken_at: i64,
    ) -> FileRecord {
        FileRecord {
            id,
            collection_id: "col-1".to_string(),
            cipher: "xchacha20-poly1305".to_string(),
            title: Some(title.to_string()),
            description: Some(desc.to_string()),
            latitude: lat,
            longitude: lon,
            taken_at: Some(taken_at),
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
    fn test_search_text() {
        let (db, _dir) = test_db();
        let files = vec![
            make_file(
                1,
                "beach sunset",
                "vacation photo",
                None,
                None,
                1700000000000,
            ),
            make_file(
                2,
                "mountain hike",
                "adventure trip",
                None,
                None,
                1700001000000,
            ),
        ];
        upsert_files(&db.conn, &files).unwrap();
        let results = search_text(&db.conn, "beach", 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, Some("beach sunset".to_string()));
    }

    #[test]
    fn test_search_by_date() {
        let (db, _dir) = test_db();
        let files = vec![
            make_file(1, "a", "b", None, None, 1700000000000),
            make_file(2, "c", "d", None, None, 1700002000000),
            make_file(3, "e", "f", None, None, 1700004000000),
        ];
        upsert_files(&db.conn, &files).unwrap();
        let results = search_by_date(&db.conn, 1700000500000, 1700003000000, 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, 2);
    }

    #[test]
    fn test_search_by_location() {
        let (db, _dir) = test_db();
        let files = vec![
            make_file(
                1,
                "nyc",
                "new york",
                Some(40.7128),
                Some(-74.0060),
                1700000000000,
            ),
            make_file(
                2,
                "la",
                "los angeles",
                Some(34.0522),
                Some(-118.2437),
                1700001000000,
            ),
        ];
        upsert_files(&db.conn, &files).unwrap();
        let results = search_by_location(&db.conn, 40.0, 41.0, -75.0, -73.0, 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, 1);
    }

    #[test]
    fn test_rebuild_fts_index() {
        let (db, _dir) = test_db();
        rebuild_fts_index(&db.conn).unwrap();
    }
}
