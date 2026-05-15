CREATE TABLE IF NOT EXISTS users (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email                   TEXT NOT NULL UNIQUE,
    verify_key_hash         TEXT NOT NULL,
    encrypted_master_key    TEXT NOT NULL,
    key_nonce               TEXT NOT NULL,
    kek_salt                TEXT NOT NULL,
    mem_limit               INTEGER NOT NULL,
    ops_limit               INTEGER NOT NULL,
    public_key              TEXT NOT NULL,
    encrypted_secret_key    TEXT NOT NULL,
    secret_key_nonce        TEXT NOT NULL,
    encrypted_recovery_key  TEXT NOT NULL,
    recovery_key_nonce      TEXT NOT NULL,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
