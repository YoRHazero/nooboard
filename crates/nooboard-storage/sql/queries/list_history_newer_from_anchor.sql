SELECT event_id, origin_noob_id, origin_device_id, created_at_ms, applied_at_ms, content, source
FROM events
WHERE state = 'active'
  AND content IS NOT NULL
  AND (
    created_at_ms > ?1
    OR (created_at_ms = ?1 AND event_id > ?2)
  )
ORDER BY created_at_ms ASC, event_id ASC
LIMIT ?3;
