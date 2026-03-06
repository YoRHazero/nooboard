SELECT event_id, origin_noob_id, origin_device_id, created_at_ms, applied_at_ms, content
FROM events
WHERE state = ?1
  AND content IS NOT NULL
  AND (
    created_at_ms < ?2
    OR (created_at_ms = ?2 AND event_id < ?3)
  )
ORDER BY created_at_ms DESC, event_id DESC
LIMIT ?4;
