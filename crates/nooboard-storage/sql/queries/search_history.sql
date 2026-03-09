SELECT event_id, origin_noob_id, origin_device_id, created_at_ms, applied_at_ms, content, source
FROM events
WHERE state = ?1 AND content IS NOT NULL AND content LIKE ?2
ORDER BY created_at_ms DESC, event_id DESC
LIMIT ?3;
