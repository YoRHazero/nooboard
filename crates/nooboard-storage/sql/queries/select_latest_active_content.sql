SELECT content
FROM events
WHERE state = ?1
ORDER BY applied_at_ms DESC, event_id DESC
LIMIT 1;
