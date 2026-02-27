UPDATE outbox_messages
SET lease_until_ms = ?1,
    updated_at_ms = ?2
WHERE id = ?3
  AND sent_at_ms IS NULL
  AND (lease_until_ms IS NULL OR lease_until_ms <= ?2);
