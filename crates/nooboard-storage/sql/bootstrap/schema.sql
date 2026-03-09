CREATE TABLE IF NOT EXISTS events (
    event_id BLOB(16) PRIMARY KEY,
    origin_noob_id TEXT NOT NULL,
    origin_device_id TEXT NOT NULL,
    created_at_ms INTEGER NOT NULL,
    applied_at_ms INTEGER NOT NULL,
    content TEXT,
    source TEXT NOT NULL CHECK (source IN ('local_capture', 'remote_sync', 'user_submit')),
    state TEXT NOT NULL CHECK (state IN ('active', 'tombstone'))
);

CREATE INDEX IF NOT EXISTS idx_events_created_at
ON events(created_at_ms DESC, event_id DESC);

CREATE INDEX IF NOT EXISTS idx_events_applied_at
ON events(applied_at_ms DESC, event_id DESC);

CREATE INDEX IF NOT EXISTS idx_events_state_created_at
ON events(state, created_at_ms);
