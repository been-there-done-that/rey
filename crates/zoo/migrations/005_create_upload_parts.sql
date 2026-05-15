CREATE TABLE IF NOT EXISTS upload_parts (
    id              BIGSERIAL PRIMARY KEY,
    upload_id       UUID NOT NULL REFERENCES uploads(id) ON DELETE CASCADE,
    part_number     SMALLINT NOT NULL,
    part_size       INTEGER NOT NULL,
    part_md5        TEXT NOT NULL,
    etag            TEXT,
    status          TEXT NOT NULL DEFAULT 'pending',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    uploaded_at     TIMESTAMPTZ,
    UNIQUE (upload_id, part_number)
);

CREATE INDEX IF NOT EXISTS idx_upload_parts_upload_id ON upload_parts(upload_id);
CREATE INDEX IF NOT EXISTS idx_upload_parts_status ON upload_parts(status);
