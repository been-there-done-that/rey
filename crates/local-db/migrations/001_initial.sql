CREATE TABLE IF NOT EXISTS collections (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,
    encrypted_key   TEXT NOT NULL,
    key_nonce       TEXT NOT NULL,
    updation_time   INTEGER NOT NULL,
    created_at      INTEGER NOT NULL,
    archived_at     INTEGER
);

CREATE TABLE IF NOT EXISTS files (
    id                      INTEGER PRIMARY KEY,
    collection_id           TEXT NOT NULL REFERENCES collections(id),
    cipher                  TEXT NOT NULL DEFAULT 'xchacha20-poly1305',
    title                   TEXT,
    description             TEXT,
    latitude                REAL,
    longitude               REAL,
    taken_at                INTEGER,
    file_size               INTEGER NOT NULL,
    mime_type               TEXT NOT NULL,
    content_hash            TEXT NOT NULL,
    encrypted_key           TEXT NOT NULL,
    key_nonce               TEXT NOT NULL,
    file_decryption_header  TEXT NOT NULL,
    thumb_decryption_header TEXT,
    object_key              TEXT NOT NULL,
    thumbnail_path          TEXT,
    updation_time           INTEGER NOT NULL,
    created_at              INTEGER NOT NULL,
    archived_at             INTEGER
);

CREATE INDEX IF NOT EXISTS idx_files_collection ON files(collection_id);
CREATE INDEX IF NOT EXISTS idx_files_taken_at ON files(taken_at);
CREATE INDEX IF NOT EXISTS idx_files_archived ON files(archived_at) WHERE archived_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_files_latitude ON files(latitude) WHERE latitude IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_files_longitude ON files(longitude) WHERE longitude IS NOT NULL;

CREATE TABLE IF NOT EXISTS sync_state (
    key     TEXT PRIMARY KEY,
    value   TEXT NOT NULL
);

PRAGMA user_version = 1;
