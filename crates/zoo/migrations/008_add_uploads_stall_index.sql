CREATE INDEX IF NOT EXISTS idx_uploads_stall_scan
    ON uploads(status, last_heartbeat_at)
    WHERE stalled_at IS NULL;
