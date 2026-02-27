CREATE TABLE IF NOT EXISTS events (
    event_id BLOB(16) PRIMARY KEY,
    origin_device_id TEXT NOT NULL,
    created_at_ms INTEGER NOT NULL,
    applied_at_ms INTEGER NOT NULL,
    content TEXT,
    state TEXT NOT NULL CHECK (state IN ('active', 'tombstone'))
);

CREATE INDEX IF NOT EXISTS idx_events_created_at
ON events(created_at_ms DESC, event_id DESC);

CREATE INDEX IF NOT EXISTS idx_events_applied_at
ON events(applied_at_ms DESC, event_id DESC);

CREATE INDEX IF NOT EXISTS idx_events_state_created_at
ON events(state, created_at_ms);

CREATE TABLE IF NOT EXISTS outbox_messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    event_id BLOB(16) NOT NULL,
    content TEXT NOT NULL,
    target_key TEXT NOT NULL,
    targets_serialized TEXT,
    attempt_count INTEGER NOT NULL DEFAULT 0,
    next_attempt_at_ms INTEGER NOT NULL,
    lease_until_ms INTEGER,
    last_error TEXT,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    sent_at_ms INTEGER,
    UNIQUE(event_id, target_key)
);

CREATE INDEX IF NOT EXISTS idx_outbox_due
ON outbox_messages(sent_at_ms, next_attempt_at_ms, lease_until_ms, created_at_ms);
