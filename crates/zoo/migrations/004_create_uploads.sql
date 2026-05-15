CREATE TABLE IF NOT EXISTS uploads (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id             UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    device_id           UUID NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    status              TEXT NOT NULL DEFAULT 'pending',
    file_hash           TEXT NOT NULL,
    file_size           BIGINT NOT NULL,
    mime_type           TEXT,
    part_size           INTEGER NOT NULL,
    part_count          SMALLINT NOT NULL,
    parts_bitmask       BYTEA,
    object_key          TEXT,
    upload_id_s3        TEXT,
    complete_url        TEXT,
    urls_expire_at      TIMESTAMPTZ,
    last_heartbeat_at   TIMESTAMPTZ DEFAULT NOW(),
    stalled_at          TIMESTAMPTZ,
    error_reason        TEXT,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at          TIMESTAMPTZ NOT NULL,
    done_at             TIMESTAMPTZ,
    UNIQUE (user_id, file_hash, (metadata->>'collection_id')) WHERE status IN ('pending', 'encrypting', 'uploading')
);

CREATE INDEX IF NOT EXISTS idx_uploads_user_id ON uploads(user_id);
CREATE INDEX IF NOT EXISTS idx_uploads_status ON uploads(status);
CREATE INDEX IF NOT EXISTS idx_uploads_stalled_at ON uploads(stalled_at) WHERE stalled_at IS NOT NULL;
