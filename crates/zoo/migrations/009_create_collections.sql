CREATE TABLE IF NOT EXISTS collections (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id             UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    encrypted_name      TEXT NOT NULL,
    encrypted_key       TEXT NOT NULL,
    key_decryption_nonce TEXT NOT NULL,
    encrypted_metadata  TEXT,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updation_time       TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_collections_user_id ON collections(user_id);
CREATE INDEX IF NOT EXISTS idx_collections_updation_time ON collections(updation_time);
