CREATE TABLE IF NOT EXISTS devices (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id                 UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name                    TEXT NOT NULL,
    platform                TEXT NOT NULL,
    sse_token               TEXT NOT NULL UNIQUE,
    push_token              TEXT,
    stall_timeout_seconds   INTEGER NOT NULL DEFAULT 90,
    is_active               BOOLEAN NOT NULL DEFAULT TRUE,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_seen_at            TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_devices_user_id ON devices(user_id);
CREATE INDEX IF NOT EXISTS idx_devices_sse_token ON devices(sse_token);
