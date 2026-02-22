CREATE TABLE IF NOT EXISTS clipboard_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    content TEXT NOT NULL,
    captured_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_clipboard_history_captured_at
ON clipboard_history(captured_at DESC, id DESC);

CREATE TABLE IF NOT EXISTS sync_seen_events (
    origin_device_id TEXT NOT NULL,
    origin_seq INTEGER NOT NULL,
    seen_at INTEGER NOT NULL,
    PRIMARY KEY (origin_device_id, origin_seq)
);
