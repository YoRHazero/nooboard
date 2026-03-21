# nooboard-storage History Paging Plan

This document is the authoritative implementation plan for directional clipboard-history paging in
`crates/nooboard-storage`.

If the storage implementation conflicts with this document, the implementation is wrong. Any
intentional behavior change must update this document first.

## 1. Goal

Unify clipboard-history queries under a single directional paging contract that can support:

- first-page loading from the history head
- paging toward older history
- paging back toward newer history
- bounded page caches in the GUI
- stable reloading of evicted history windows

This redesign MUST keep `HistoryRecord` as the row payload. It MUST NOT introduce a second
summary-only history DTO.

## 2. Non-Negotiable Rules

- History paging MUST use one interface shape: `direction + anchor + limit`.
- The returned `records` order MUST always be `created_at_ms DESC, event_id DESC`.
- `anchor` is a page-boundary locator, not a scroll position.
- `Older + anchor=None` means "load the head page".
- `Newer + anchor=None` is invalid and MUST return an error.
- `next_anchor` MUST mean "the anchor to continue paging in the same direction".
- Storage MUST continue returning full `HistoryRecord` rows.
- Storage MUST NOT keep the old cursor-based interface in parallel once the new interface lands.

## 3. Public Storage Types

`crates/nooboard-storage/src/model.rs` MUST expose these history-paging types:

```rust
pub enum HistoryDirection {
    Older,
    Newer,
}

pub struct HistoryAnchor {
    pub created_at_ms: i64,
    pub event_id: [u8; 16],
}

pub struct ListHistoryRequest {
    pub limit: usize,
    pub direction: HistoryDirection,
    pub anchor: Option<HistoryAnchor>,
}

pub struct HistoryPage {
    pub records: Vec<HistoryRecord>,
    pub has_more: bool,
    pub next_anchor: Option<HistoryAnchor>,
}
```

`HistoryRecord` MUST expose:

```rust
impl HistoryRecord {
    pub fn anchor(&self) -> HistoryAnchor;
}
```

The legacy `HistoryCursor` type MUST be deleted.

## 4. Repository Contract

`SqliteEventRepository` MUST expose this interface:

```rust
pub fn list_history(&self, request: ListHistoryRequest) -> Result<HistoryPage, StorageError>;
```

Semantics:

- `Older + None`:
  - return the newest page
- `Older + Some(anchor)`:
  - return records strictly older than `anchor`
- `Newer + Some(anchor)`:
  - return records strictly newer than `anchor`
- `Newer + None`:
  - return `StorageError::InvalidHistoryRequest(...)`

The repository MUST overfetch by one row to compute `has_more`.

After trimming the extra row:

- `Older` MUST set `next_anchor = records.last().map(HistoryRecord::anchor)`
- `Newer` MUST set `next_anchor = records.first().map(HistoryRecord::anchor)`

If the returned `records` are empty, `next_anchor` MUST be `None`.

## 5. SQL Shape

The SQL catalog MUST split history paging into three explicit queries:

- `list_history_head.sql`
- `list_history_older_from_anchor.sql`
- `list_history_newer_from_anchor.sql`

### 5.1 Head query

```sql
SELECT event_id, origin_noob_id, origin_device_id, created_at_ms, applied_at_ms, content, source
FROM events
WHERE state = 'active' AND content IS NOT NULL
ORDER BY created_at_ms DESC, event_id DESC
LIMIT ?1;
```

### 5.2 Older query

```sql
SELECT event_id, origin_noob_id, origin_device_id, created_at_ms, applied_at_ms, content, source
FROM events
WHERE state = 'active'
  AND content IS NOT NULL
  AND (
    created_at_ms < ?1
    OR (created_at_ms = ?1 AND event_id < ?2)
  )
ORDER BY created_at_ms DESC, event_id DESC
LIMIT ?3;
```

### 5.3 Newer query

```sql
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
```

The repository MUST reverse the `Newer` query result before building `HistoryPage` so the public
return order remains `DESC`.

History queries MUST hardcode `state = 'active'`. They MUST NOT keep `state` as a bound parameter.

## 6. Required Index Set

The final secondary-index set MUST be:

```sql
CREATE INDEX IF NOT EXISTS idx_events_history_state_created_at_event
ON events(state, created_at_ms DESC, event_id DESC)
WHERE content IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_events_latest_state_applied_at
ON events(state, applied_at_ms DESC, event_id DESC);

CREATE INDEX IF NOT EXISTS idx_events_gc_state_created_at
ON events(state, created_at_ms ASC);
```

The old indexes below MUST be removed or replaced:

- `idx_events_created_at`
- `idx_events_applied_at`
- `idx_events_state_created_at`

Rationale:

- history paging and history search need `state + created_at + event_id` ordering
- latest-active lookup needs `state + applied_at + event_id` ordering
- GC needs a separate `state + created_at` path and must not share the history index

## 7. Search-History Behavior

`search_history.sql` MAY keep `content LIKE ?` filtering, but it MUST use the same final ordering:

- `ORDER BY created_at_ms DESC, event_id DESC`

It SHOULD continue to benefit from `idx_events_history_state_created_at_event`, even though the
`LIKE` filter still requires row inspection.

## 8. Error Handling

`StorageError` MUST gain an invalid-history-request variant for bad directional calls such as
`Newer + None`.

Storage MUST fail explicitly rather than silently treating invalid directional requests as head-page
loads.

## 9. Tests

The storage test suite MUST cover at least:

- head-page fetch returns newest-first order
- `Older` paging walks from newer pages to older pages without duplication
- `Newer` paging walks from older pages back toward newer pages without duplication
- ties on `created_at_ms` are broken by `event_id`
- `has_more` is true only when an overfetched row exists
- `Newer + None` returns an error
- GC and latest-active queries still behave correctly after index replacement

## 10. Downstream Contract

`nooboard-core` MUST mirror this storage contract with clipboard-specific types:

- `ClipboardHistoryDirection`
- `ClipboardHistoryAnchor`
- `ListClipboardHistoryRequest { limit, direction, anchor }`
- `ClipboardHistoryPage { records, has_more, next_anchor }`

`nooboard-gui` MUST build bounded page caching and virtualized clipboard history on top of this
directional paging contract. It MUST NOT reintroduce a single unbounded `next_cursor` list model
after this storage redesign lands.

The first bounded-window GUI implementation SHOULD use explicit gap rows for unloaded ranges:

- a top gap row for unloaded newer history
- a bottom gap row for unloaded older history
- filling a gap MUST use the matching `Newer` or `Older` directional request instead of rebuilding
  the full list from the head
