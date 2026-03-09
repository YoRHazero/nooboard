INSERT OR IGNORE INTO events (
    event_id,
    origin_noob_id,
    origin_device_id,
    created_at_ms,
    applied_at_ms,
    content,
    source,
    state
)
VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8);
