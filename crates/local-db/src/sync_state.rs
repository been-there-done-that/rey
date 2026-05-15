use crate::error::LocalDbError;
use rusqlite::{Connection, OptionalExtension};

pub fn read_cursor(conn: &Connection, key: &str) -> Result<Option<i64>, LocalDbError> {
    let mut stmt = conn.prepare("SELECT value FROM sync_state WHERE key = ?1")?;
    let value: Option<String> = stmt.query_row([key], |row| row.get(0)).optional()?;
    match value {
        Some(v) => Ok(Some(v.parse().unwrap_or(0))),
        None => Ok(None),
    }
}

pub fn write_cursor(conn: &Connection, key: &str, value: i64) -> Result<(), LocalDbError> {
    conn.execute(
        "INSERT INTO sync_state (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = ?2",
        (key, value.to_string()),
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
    fn test_read_write_cursor() {
        let (db, _dir) = test_db();
        assert!(read_cursor(&db.conn, "collections_since")
            .unwrap()
            .is_none());
        write_cursor(&db.conn, "collections_since", 1700000000000).unwrap();
        let value = read_cursor(&db.conn, "collections_since").unwrap().unwrap();
        assert_eq!(value, 1700000000000);
    }

    #[test]
    fn test_write_cursor_updates_existing() {
        let (db, _dir) = test_db();
        write_cursor(&db.conn, "trash_since", 100).unwrap();
        write_cursor(&db.conn, "trash_since", 200).unwrap();
        let value = read_cursor(&db.conn, "trash_since").unwrap().unwrap();
        assert_eq!(value, 200);
    }
}
