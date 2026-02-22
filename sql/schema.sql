CREATE TABLE IF NOT EXISTS clipboard_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    content TEXT NOT NULL,
    captured_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_clipboard_history_captured_at
ON clipboard_history(captured_at DESC, id DESC);
