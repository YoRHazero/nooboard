INSERT OR IGNORE INTO outbox_messages (
    event_id,
    content,
    target_key,
    targets_serialized,
    attempt_count,
    next_attempt_at_ms,
    lease_until_ms,
    last_error,
    created_at_ms,
    updated_at_ms,
    sent_at_ms
)
VALUES (?1, ?2, ?3, ?4, 0, ?5, NULL, NULL, ?5, ?5, NULL);
