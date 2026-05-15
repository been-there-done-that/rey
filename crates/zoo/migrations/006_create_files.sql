CREATE TABLE IF NOT EXISTS files (
    id                      BIGSERIAL PRIMARY KEY,
    user_id                 UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    collection_id           TEXT NOT NULL,
    cipher                  TEXT NOT NULL DEFAULT 'xchacha20-poly1305',
    encrypted_key           TEXT NOT NULL,
    key_decryption_nonce    TEXT NOT NULL,
    file_decryption_header  TEXT NOT NULL,
    thumb_decryption_header TEXT,
    encrypted_metadata      TEXT NOT NULL,
    encrypted_thumbnail     TEXT,
    thumbnail_size          INTEGER,
    file_size               BIGINT NOT NULL,
    mime_type               TEXT NOT NULL,
    content_hash            TEXT NOT NULL,
    object_key              TEXT NOT NULL,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updation_time           TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    archived_at             TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_files_user_id ON files(user_id);
CREATE INDEX IF NOT EXISTS idx_files_collection_id ON files(user_id, collection_id);
CREATE INDEX IF NOT EXISTS idx_files_updation_time ON files(updation_time);
CREATE INDEX IF NOT EXISTS idx_files_archived ON files(archived_at) WHERE archived_at IS NULL;
