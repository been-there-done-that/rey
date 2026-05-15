CREATE TABLE IF NOT EXISTS shares (
    id                          BIGSERIAL PRIMARY KEY,
    file_id                     BIGINT NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    shared_with                 UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    collection_id               TEXT NOT NULL,
    encrypted_collection_key    TEXT NOT NULL,
    created_at                  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at                  TIMESTAMPTZ,
    UNIQUE (file_id, shared_with)
);

CREATE INDEX IF NOT EXISTS idx_shares_shared_with ON shares(shared_with);
