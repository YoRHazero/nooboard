UPDATE outbox_messages
SET attempt_count = attempt_count + 1,
    next_attempt_at_ms = ?1,
    lease_until_ms = NULL,
    last_error = ?2,
    updated_at_ms = ?3
WHERE id = ?4
  AND sent_at_ms IS NULL;
