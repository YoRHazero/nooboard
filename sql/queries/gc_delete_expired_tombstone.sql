DELETE FROM events
WHERE event_id IN (
    SELECT event_id
    FROM events
    WHERE state = ?1 AND created_at_ms <= ?2
    ORDER BY created_at_ms ASC
    LIMIT ?3
);
