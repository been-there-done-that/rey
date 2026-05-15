use rusqlite::{Connection, OpenFlags};
use std::path::Path;
use crate::error::LocalDbError;

const MIGRATIONS: &[&str] = &[
    include_str!("../migrations/001_initial.sql"),
    include_str!("../migrations/002_fts5.sql"),
];

pub struct LocalDb {
    pub conn: Connection,
}

impl LocalDb {
    pub fn open_with_key(db_path: &Path, key: &[u8; 32]) -> Result<Self, LocalDbError> {
        std::fs::create_dir_all(db_path.parent().unwrap_or(Path::new(".")))
            .map_err(LocalDbError::Io)?;

        let conn = Connection::open_with_flags(
            db_path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
        )?;

        let hex_key = hex::encode(key);
        conn.pragma_update(None, "key", &hex_key)?;

        conn.pragma_update(None, "journal_mode", "wal")?;
        conn.pragma_update(None, "synchronous", "normal")?;

        let version: Result<i32, _> = conn.pragma_query_value(None, "user_version", |row| row.get(0));
        match version {
            Ok(v) if v >= 0 => {}
            _ => return Err(LocalDbError::InvalidKey),
        }

        Self::run_migrations(&conn)?;

        Ok(Self { conn })
    }

    pub fn open(db_path: &Path) -> Result<Self, LocalDbError> {
        let key = Self::retrieve_or_generate_key()?;
        Self::open_with_key(db_path, &key)
    }

    fn retrieve_or_generate_key() -> Result<[u8; 32], LocalDbError> {
        let entry = keyring::Entry::new("rey", "local_db_key")
            .map_err(|_| LocalDbError::KeychainUnavailable)?;

        match entry.get_password() {
            Ok(hex_key) => {
                let bytes = hex::decode(&hex_key).map_err(|_| LocalDbError::KeychainUnavailable)?;
                let mut key = [0u8; 32];
                key.copy_from_slice(&bytes);
                Ok(key)
            }
            Err(keyring::Error::NoEntry) => {
                let mut key = [0u8; 32];
                getrandom::fill(&mut key).map_err(|_| LocalDbError::KeychainUnavailable)?;
                let hex_key = hex::encode(key);
                entry
                    .set_password(&hex_key)
                    .map_err(|_| LocalDbError::KeychainUnavailable)?;
                Ok(key)
            }
            Err(_) => Err(LocalDbError::KeychainUnavailable),
        }
    }

    fn run_migrations(conn: &Connection) -> Result<(), LocalDbError> {
        let current_version: i32 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(LocalDbError::QueryError)?;

        for (i, migration) in MIGRATIONS.iter().enumerate() {
            let target_version = (i + 1) as i32;
            if current_version >= target_version {
                continue;
            }

            conn.execute_batch(migration)
                .map_err(|e| LocalDbError::MigrationFailed(format!("migration {}: {}", i + 1, e)))?;
        }

        Ok(())
    }

    #[cfg(test)]
    pub fn open_test(db_path: &Path) -> Result<Self, LocalDbError> {
        use rand::RngCore;
        let mut key = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut key);
        let db = Self::open_with_key(db_path, &key)?;
        db.conn.execute_batch("PRAGMA foreign_keys = OFF")?;
        Ok(db)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_open_creates_database() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let db = LocalDb::open_test(&db_path).unwrap();
        assert!(db_path.exists());
        drop(db);
    }

    #[test]
    fn test_wrong_key_fails() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let key1 = [1u8; 32];
        let db = LocalDb::open_with_key(&db_path, &key1).unwrap();
        drop(db);

        let key2 = [2u8; 32];
        let result = LocalDb::open_with_key(&db_path, &key2);
        assert!(result.is_err());
    }
}
