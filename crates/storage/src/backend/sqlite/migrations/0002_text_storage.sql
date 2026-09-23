CREATE TABLE history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    text TEXT NOT NULL CHECK(typeof(text) = 'text'),
    text_hash BLOB NOT NULL CHECK(length(text_hash) = 32),
    source_kind INTEGER NOT NULL CHECK(source_kind IN (0, 1)),
    source_id TEXT,
    copied_at_ms INTEGER NOT NULL CHECK(copied_at_ms >= 0),
    CHECK((source_kind = 0 AND source_id IS NULL) OR
          (source_kind = 1 AND source_id IS NOT NULL AND length(source_id) > 0))
);
-- The digest narrows candidates; full text equality decides deduplication.
-- It is deliberately non-unique so different texts can share a digest.
CREATE INDEX history_digest ON history(text_hash);
CREATE INDEX history_recent ON history(copied_at_ms DESC, id DESC);
CREATE TABLE settings (
    key TEXT PRIMARY KEY NOT NULL CHECK(typeof(key) = 'text' AND length(key) > 0),
    value TEXT NOT NULL CHECK(typeof(value) = 'text')
);
