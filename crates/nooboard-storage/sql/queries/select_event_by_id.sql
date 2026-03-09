SELECT event_id, origin_noob_id, origin_device_id, created_at_ms, applied_at_ms, content, source
FROM events
WHERE event_id = ?1
  AND state = ?2
  AND content IS NOT NULL
LIMIT 1;
