UPDATE outbox_messages
SET sent_at_ms = ?1,
    lease_until_ms = NULL,
    last_error = NULL,
    updated_at_ms = ?1
WHERE id = ?2;
