SELECT
    id,
    event_id,
    content,
    target_key,
    targets_serialized,
    attempt_count,
    next_attempt_at_ms
FROM outbox_messages
WHERE sent_at_ms IS NULL
  AND next_attempt_at_ms <= ?1
  AND (lease_until_ms IS NULL OR lease_until_ms <= ?1)
ORDER BY created_at_ms ASC, id ASC
LIMIT ?2;
