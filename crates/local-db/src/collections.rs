use crate::error::LocalDbError;
use common::time::now_ms;
use rusqlite::{Connection, OptionalExtension};
use types::collection::Collection;

pub fn upsert_collection(conn: &Connection, collection: &Collection) -> Result<(), LocalDbError> {
    conn.execute(
        "INSERT INTO collections (id, name, encrypted_key, key_nonce, updation_time, created_at, archived_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(id) DO UPDATE SET
             name = ?2, encrypted_key = ?3, key_nonce = ?4,
             updation_time = ?5, created_at = ?6, archived_at = ?7",
        (
            &collection.id,
            &collection.name,
            &collection.encrypted_key,
            &collection.key_nonce,
            collection.updation_time,
            collection.created_at,
            collection.archived_at,
        ),
    )?;
    Ok(())
}

pub fn list_collections(conn: &Connection) -> Result<Vec<Collection>, LocalDbError> {
    let mut stmt = conn.prepare(
        "SELECT id, name, encrypted_key, key_nonce, updation_time, created_at, archived_at
         FROM collections WHERE archived_at IS NULL",
    )?;
    let collections = stmt
        .query_map([], |row| {
            Ok(Collection {
                id: row.get(0)?,
                name: row.get(1)?,
                encrypted_key: row.get(2)?,
                key_nonce: row.get(3)?,
                updation_time: row.get(4)?,
                created_at: row.get(5)?,
                archived_at: row.get(6)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(collections)
}

pub fn get_collection_key(
    conn: &Connection,
    id: &str,
) -> Result<Option<(String, String)>, LocalDbError> {
    let mut stmt =
        conn.prepare("SELECT encrypted_key, key_nonce FROM collections WHERE id = ?1")?;
    let result = stmt
        .query_row([id], |row| Ok((row.get(0)?, row.get(1)?)))
        .optional()?;
    Ok(result)
}

pub fn archive_collection(conn: &Connection, id: &str) -> Result<(), LocalDbError> {
    let now = now_ms();
    conn.execute(
        "UPDATE collections SET archived_at = ?1 WHERE id = ?2",
        (now, id),
    )?;
    Ok(())
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

    #[test]
    fn test_upsert_and_list_collections() {
        let (db, _dir) = test_db();
        let col = Collection {
            id: "col-1".to_string(),
            name: "Vacation".to_string(),
            encrypted_key: "ek".to_string(),
            key_nonce: "kn".to_string(),
            updation_time: 1700000000000,
            created_at: 1700000000000,
            archived_at: None,
        };
        upsert_collection(&db.conn, &col).unwrap();
        let collections = list_collections(&db.conn).unwrap();
        assert_eq!(collections.len(), 1);
        assert_eq!(collections[0].name, "Vacation");
    }

    #[test]
    fn test_archive_collection() {
        let (db, _dir) = test_db();
        let col = Collection {
            id: "col-1".to_string(),
            name: "Vacation".to_string(),
            encrypted_key: "ek".to_string(),
            key_nonce: "kn".to_string(),
            updation_time: 1700000000000,
            created_at: 1700000000000,
            archived_at: None,
        };
        upsert_collection(&db.conn, &col).unwrap();
        archive_collection(&db.conn, "col-1").unwrap();
        let collections = list_collections(&db.conn).unwrap();
        assert!(collections.is_empty());
    }

    #[test]
    fn test_get_collection_key() {
        let (db, _dir) = test_db();
        let col = Collection {
            id: "col-1".to_string(),
            name: "Vacation".to_string(),
            encrypted_key: "ek".to_string(),
            key_nonce: "kn".to_string(),
            updation_time: 1700000000000,
            created_at: 1700000000000,
            archived_at: None,
        };
        upsert_collection(&db.conn, &col).unwrap();
        let key = get_collection_key(&db.conn, "col-1").unwrap();
        assert!(key.is_some());
        let (ek, kn) = key.unwrap();
        assert_eq!(ek, "ek");
        assert_eq!(kn, "kn");
    }
}
