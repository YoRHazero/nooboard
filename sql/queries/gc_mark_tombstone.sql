UPDATE events
SET content = NULL,
    state = ?2,
    applied_at_ms = ?3
WHERE event_id IN (
    SELECT event_id
    FROM events
    WHERE state = ?1 AND created_at_ms <= ?4
    ORDER BY created_at_ms ASC
    LIMIT ?5
);
