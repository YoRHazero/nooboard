use std::fs;
use std::path::Path;

use rusqlite::{Connection, OptionalExtension, Row, params, types::Type};

use crate::StorageError;
use crate::config::{AppConfig, STORAGE_SCHEMA_VERSION, StorageConfig};
use crate::model::{EventState, HistoryCursor, HistoryRecord, OutboxMessage};
use crate::sql_catalog::SqlCatalog;

pub struct SqliteEventRepository {
    conn: Connection,
    storage: StorageConfig,
    sql: SqlCatalog,
    local_device_id: String,
    inserts_since_gc: usize,
}

impl SqliteEventRepository {
    pub fn open_from_config(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        let config = AppConfig::load(path)?;
        Self::open(config)
    }

    pub fn open(config: AppConfig) -> Result<Self, StorageError> {
        config.validate()?;
        let storage = config.storage;

        fs::create_dir_all(storage.current_version_dir())?;

        let sql = SqlCatalog::load();
        let conn = Connection::open(storage.db_path())?;

        Ok(Self {
            conn,
            storage,
            sql,
            local_device_id: resolve_local_device_id(),
            inserts_since_gc: 0,
        })
    }

    pub fn init_storage(&mut self) -> Result<(), StorageError> {
        fs::create_dir_all(self.storage.current_version_dir())?;
        self.conn.execute_batch(&self.sql.schema)?;
        prune_old_versions(&self.storage)?;
        Ok(())
    }

    pub fn append_text(
        &mut self,
        text: &str,
        event_id: Option<uuid::Uuid>,
        origin_device_id: Option<&str>,
        created_at_ms: i64,
        applied_at_ms: i64,
    ) -> Result<bool, StorageError> {
        let inserted = self.insert_text_event(
            text,
            event_id,
            origin_device_id,
            created_at_ms,
            applied_at_ms,
        )?;

        if !inserted {
            return Ok(false);
        }

        self.inserts_since_gc = self.inserts_since_gc.saturating_add(1);
        self.run_gc_if_needed(applied_at_ms)?;

        Ok(true)
    }

    pub fn append_text_with_outbox(
        &mut self,
        text: &str,
        event_id: uuid::Uuid,
        origin_device_id: Option<&str>,
        created_at_ms: i64,
        applied_at_ms: i64,
        targets: Option<&[String]>,
        enqueue_at_ms: i64,
    ) -> Result<bool, StorageError> {
        let inserted = self.insert_text_event(
            text,
            Some(event_id),
            origin_device_id,
            created_at_ms,
            applied_at_ms,
        )?;
        if !inserted {
            return Ok(false);
        }

        for (target_key, targets_serialized) in normalize_targets_for_outbox(targets) {
            self.conn.execute(
                &self.sql.outbox_enqueue,
                params![
                    event_id.as_bytes().as_slice(),
                    text,
                    target_key,
                    targets_serialized,
                    enqueue_at_ms
                ],
            )?;
        }

        self.inserts_since_gc = self.inserts_since_gc.saturating_add(1);
        self.run_gc_if_needed(applied_at_ms)?;
        Ok(true)
    }

    pub fn list_history(
        &self,
        limit: usize,
        cursor: Option<HistoryCursor>,
    ) -> Result<Vec<HistoryRecord>, StorageError> {
        if limit == 0 {
            return Ok(Vec::new());
        }

        let limit = i64::try_from(limit).map_err(|_| StorageError::LimitOutOfRange(limit))?;
        match cursor {
            None => {
                let mut statement = self.conn.prepare(&self.sql.list_history)?;
                let rows = statement
                    .query_map(params![EventState::Active.as_str(), limit], map_history_row)?;
                rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
            }
            Some(cursor) => {
                let mut statement = self.conn.prepare(&self.sql.list_history_with_cursor)?;
                let rows = statement.query_map(
                    params![
                        EventState::Active.as_str(),
                        cursor.created_at_ms,
                        &cursor.event_id[..],
                        limit
                    ],
                    map_history_row,
                )?;
                rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
            }
        }
    }

    pub fn search_history(
        &self,
        limit: usize,
        keyword: &str,
    ) -> Result<Vec<HistoryRecord>, StorageError> {
        if keyword.trim().is_empty() {
            return self.list_history(limit, None);
        }
        if limit == 0 {
            return Ok(Vec::new());
        }

        let limit = i64::try_from(limit).map_err(|_| StorageError::LimitOutOfRange(limit))?;
        let pattern = format!("%{keyword}%");

        let mut statement = self.conn.prepare(&self.sql.search_history)?;
        let rows = statement.query_map(
            params![EventState::Active.as_str(), pattern, limit],
            map_history_row,
        )?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn list_due_outbox(
        &self,
        now_ms: i64,
        limit: usize,
    ) -> Result<Vec<OutboxMessage>, StorageError> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let limit = i64::try_from(limit).map_err(|_| StorageError::LimitOutOfRange(limit))?;
        let mut statement = self.conn.prepare(&self.sql.outbox_list_due)?;
        let rows = statement.query_map(params![now_ms, limit], map_outbox_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn try_lease_outbox_message(
        &self,
        id: i64,
        lease_until_ms: i64,
        now_ms: i64,
    ) -> Result<bool, StorageError> {
        let updated = self.conn.execute(
            &self.sql.outbox_try_lease,
            params![lease_until_ms, now_ms, id],
        )?;
        Ok(updated > 0)
    }

    pub fn mark_outbox_sent(&self, id: i64, sent_at_ms: i64) -> Result<bool, StorageError> {
        let updated = self
            .conn
            .execute(&self.sql.outbox_mark_sent, params![sent_at_ms, id])?;
        Ok(updated > 0)
    }

    pub fn mark_outbox_retry(
        &self,
        id: i64,
        next_attempt_at_ms: i64,
        error: &str,
        now_ms: i64,
    ) -> Result<bool, StorageError> {
        let updated = self.conn.execute(
            &self.sql.outbox_mark_retry,
            params![next_attempt_at_ms, error, now_ms, id],
        )?;
        Ok(updated > 0)
    }

    pub fn run_gc_if_needed(&mut self, now_ms: i64) -> Result<(), StorageError> {
        if self.inserts_since_gc < self.storage.lifecycle.gc_every_inserts as usize {
            return Ok(());
        }

        self.inserts_since_gc = 0;

        let history_cutoff_ms = cutoff_ms(now_ms, self.storage.lifecycle.history_window_days);
        let dedup_cutoff_ms = cutoff_ms(now_ms, self.storage.lifecycle.dedup_window_days);
        let batch_size = i64::from(self.storage.lifecycle.gc_batch_size);

        let transaction = self.conn.transaction()?;
        transaction.execute(
            &self.sql.gc_mark_tombstone,
            params![
                EventState::Active.as_str(),
                EventState::Tombstone.as_str(),
                now_ms,
                history_cutoff_ms,
                batch_size
            ],
        )?;
        transaction.execute(
            &self.sql.gc_delete_expired_tombstone,
            params![EventState::Tombstone.as_str(), dedup_cutoff_ms, batch_size],
        )?;
        transaction.commit()?;

        Ok(())
    }

    fn latest_active_content(&self) -> Result<Option<String>, StorageError> {
        self.conn
            .query_row(
                &self.sql.select_latest_active_content,
                [EventState::Active.as_str()],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(Into::into)
    }

    fn insert_text_event(
        &mut self,
        text: &str,
        event_id: Option<uuid::Uuid>,
        origin_device_id: Option<&str>,
        created_at_ms: i64,
        applied_at_ms: i64,
    ) -> Result<bool, StorageError> {
        let should_skip_duplicate = event_id.is_none() && origin_device_id.is_none();
        if should_skip_duplicate {
            let latest_content = self.latest_active_content()?;
            if latest_content.as_deref() == Some(text) {
                return Ok(false);
            }
        }

        let event_id = event_id.unwrap_or_else(uuid::Uuid::now_v7);
        let origin_device_id = origin_device_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(self.local_device_id.as_str());

        let inserted = self.conn.execute(
            &self.sql.insert_event,
            params![
                event_id.as_bytes().as_slice(),
                origin_device_id,
                created_at_ms,
                applied_at_ms,
                text,
                EventState::Active.as_str(),
            ],
        )? > 0;

        Ok(inserted)
    }
}

fn map_history_row(row: &Row<'_>) -> rusqlite::Result<HistoryRecord> {
    let event_id_blob: Vec<u8> = row.get(0)?;
    let event_id = event_id_blob.try_into().map_err(|value: Vec<u8>| {
        rusqlite::Error::FromSqlConversionFailure(
            0,
            Type::Blob,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("event_id must be exactly 16 bytes, got {}", value.len()),
            )),
        )
    })?;

    Ok(HistoryRecord {
        event_id,
        origin_device_id: row.get(1)?,
        created_at_ms: row.get(2)?,
        applied_at_ms: row.get(3)?,
        content: row.get(4)?,
    })
}

fn map_outbox_row(row: &Row<'_>) -> rusqlite::Result<OutboxMessage> {
    let event_id_blob: Vec<u8> = row.get(1)?;
    let event_id = event_id_blob.try_into().map_err(|value: Vec<u8>| {
        rusqlite::Error::FromSqlConversionFailure(
            1,
            Type::Blob,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("event_id must be exactly 16 bytes, got {}", value.len()),
            )),
        )
    })?;

    let targets_serialized: Option<String> = row.get(4)?;
    let targets = decode_targets(targets_serialized.as_deref());
    let attempt_count_i64: i64 = row.get(5)?;
    let attempt_count = if attempt_count_i64 < 0 {
        0
    } else {
        u32::try_from(attempt_count_i64).unwrap_or(u32::MAX)
    };

    Ok(OutboxMessage {
        id: row.get(0)?,
        event_id,
        content: row.get(2)?,
        target_key: row.get(3)?,
        targets,
        attempt_count,
        next_attempt_at_ms: row.get(6)?,
    })
}

fn normalize_targets_for_outbox(targets: Option<&[String]>) -> Vec<(String, Option<String>)> {
    let Some(normalized) = normalize_targets(targets) else {
        return Vec::new();
    };
    match normalized {
        None => vec![("all".to_string(), None)],
        Some(nodes) => nodes
            .into_iter()
            .map(|node| (format!("node:{node}"), Some(node)))
            .collect(),
    }
}

fn normalize_targets(targets: Option<&[String]>) -> Option<Option<Vec<String>>> {
    match targets {
        None => Some(None),
        Some(nodes) => {
            let mut normalized: Vec<String> = nodes
                .iter()
                .map(|node| node.trim().to_string())
                .filter(|node| !node.is_empty())
                .collect();
            normalized.sort();
            normalized.dedup();
            if normalized.is_empty() {
                None
            } else {
                Some(Some(normalized))
            }
        }
    }
}

fn decode_targets(serialized: Option<&str>) -> Option<Vec<String>> {
    let value = serialized?;
    let nodes: Vec<String> = value
        .split('\n')
        .map(str::trim)
        .filter(|node| !node.is_empty())
        .map(ToString::to_string)
        .collect();
    if nodes.is_empty() { None } else { Some(nodes) }
}

fn cutoff_ms(now_ms: i64, window_days: u32) -> i64 {
    const DAY_MS: i64 = 24 * 60 * 60 * 1000;
    let window_ms = i64::from(window_days) * DAY_MS;
    now_ms.saturating_sub(window_ms)
}

fn resolve_local_device_id() -> String {
    std::env::var("NOOBOARD_DEVICE_ID")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| std::env::var("HOSTNAME").ok())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "local-device".to_string())
}

fn prune_old_versions(storage: &StorageConfig) -> Result<(), StorageError> {
    if storage.retain_old_versions != 0 {
        return Ok(());
    }

    if !storage.db_root.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(&storage.db_root)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }

        let directory_name = entry.file_name();
        if directory_name == STORAGE_SCHEMA_VERSION {
            continue;
        }

        fs::remove_dir_all(entry.path())?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::process;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use crate::config::LifecycleConfig;

    fn temp_db_root(name: &str) -> std::path::PathBuf {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or(0);

        std::env::temp_dir().join(format!(
            "nooboard-storage-{name}-{}-{millis}",
            process::id()
        ))
    }

    fn make_config(
        name: &str,
        lifecycle: LifecycleConfig,
        retain_old_versions: usize,
    ) -> AppConfig {
        AppConfig {
            storage: StorageConfig {
                db_root: temp_db_root(name),
                retain_old_versions,
                lifecycle,
            },
        }
    }

    #[test]
    fn init_storage_removes_old_versions_when_retention_is_zero() -> Result<(), StorageError> {
        let config = make_config("retain-zero", LifecycleConfig::default(), 0);
        let old_version_dir = config.storage.db_root.join("v0.0.1");
        fs::create_dir_all(&old_version_dir)?;

        let mut repository = SqliteEventRepository::open(config.clone())?;
        repository.init_storage()?;

        assert!(config.storage.current_version_dir().exists());
        assert!(!old_version_dir.exists());

        let _ = fs::remove_dir_all(config.storage.db_root);
        Ok(())
    }

    #[test]
    fn append_and_list_history_returns_latest_first() -> Result<(), StorageError> {
        let mut repository =
            SqliteEventRepository::open(make_config("append-list", LifecycleConfig::default(), 0))?;
        repository.init_storage()?;

        assert!(repository.append_text("first", None, None, 100, 100)?);
        assert!(repository.append_text("second", None, None, 200, 200)?);

        let records = repository.list_history(10, None)?;
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].content, "second");
        assert_eq!(records[0].created_at_ms, 200);
        assert_eq!(records[1].content, "first");
        assert_eq!(records[1].created_at_ms, 100);

        let _ = fs::remove_dir_all(repository.storage.db_root.clone());
        Ok(())
    }

    #[test]
    fn search_history_filters_by_keyword() -> Result<(), StorageError> {
        let mut repository =
            SqliteEventRepository::open(make_config("search", LifecycleConfig::default(), 0))?;
        repository.init_storage()?;

        assert!(repository.append_text("alpha", None, None, 100, 100)?);
        assert!(repository.append_text("beta", None, None, 200, 200)?);
        assert!(repository.append_text("alphabet", None, None, 300, 300)?);

        let records = repository.search_history(10, "alpha")?;
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].content, "alphabet");
        assert_eq!(records[1].content, "alpha");

        let _ = fs::remove_dir_all(repository.storage.db_root.clone());
        Ok(())
    }

    #[test]
    fn append_skips_consecutive_duplicate_text() -> Result<(), StorageError> {
        let mut repository =
            SqliteEventRepository::open(make_config("dedup", LifecycleConfig::default(), 0))?;
        repository.init_storage()?;

        assert!(repository.append_text("dup", None, None, 100, 100)?);
        assert!(!repository.append_text("dup", None, None, 200, 200)?);

        let records = repository.list_history(10, None)?;
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].content, "dup");

        let _ = fs::remove_dir_all(repository.storage.db_root.clone());
        Ok(())
    }

    #[test]
    fn gc_hides_expired_history_from_queries() -> Result<(), StorageError> {
        let lifecycle = LifecycleConfig {
            history_window_days: 1,
            dedup_window_days: 2,
            gc_every_inserts: 1,
            gc_batch_size: 100,
        };

        let mut repository = SqliteEventRepository::open(make_config("gc", lifecycle, 0))?;
        repository.init_storage()?;

        const DAY_MS: i64 = 24 * 60 * 60 * 1000;
        let now_ms = 10 * DAY_MS;

        assert!(repository.append_text(
            "expired",
            None,
            None,
            now_ms - 3 * DAY_MS,
            now_ms - 3 * DAY_MS
        )?);
        assert!(repository.append_text(
            "tombstone",
            None,
            None,
            now_ms - (DAY_MS + DAY_MS / 2),
            now_ms - (DAY_MS + DAY_MS / 2)
        )?);
        assert!(repository.append_text("fresh", None, None, now_ms, now_ms)?);

        let records = repository.list_history(10, None)?;
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].content, "fresh");

        let _ = fs::remove_dir_all(repository.storage.db_root.clone());
        Ok(())
    }

    #[test]
    fn list_history_with_cursor_pages_from_new_to_old() -> Result<(), StorageError> {
        let mut repository =
            SqliteEventRepository::open(make_config("cursor-page", LifecycleConfig::default(), 0))?;
        repository.init_storage()?;

        assert!(repository.append_text("first", None, None, 100, 100)?);
        assert!(repository.append_text("second", None, None, 200, 200)?);
        assert!(repository.append_text("third", None, None, 300, 300)?);

        let first_page = repository.list_history(2, None)?;
        assert_eq!(first_page.len(), 2);
        assert_eq!(first_page[0].content, "third");
        assert_eq!(first_page[1].content, "second");

        let cursor = first_page[1].cursor();
        let second_page = repository.list_history(2, Some(cursor))?;
        assert_eq!(second_page.len(), 1);
        assert_eq!(second_page[0].content, "first");

        let _ = fs::remove_dir_all(repository.storage.db_root.clone());
        Ok(())
    }

    #[test]
    fn append_text_accepts_explicit_event_id_and_origin_device() -> Result<(), StorageError> {
        let mut repository = SqliteEventRepository::open(make_config(
            "append-explicit-event",
            LifecycleConfig::default(),
            0,
        ))?;
        repository.init_storage()?;

        let event_id = uuid::Uuid::now_v7();
        assert!(repository.append_text(
            "remote-text",
            Some(event_id),
            Some("remote-node"),
            100,
            100
        )?);

        let records = repository.list_history(10, None)?;
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].event_id, *event_id.as_bytes());
        assert_eq!(records[0].origin_device_id, "remote-node");

        assert!(!repository.append_text(
            "remote-text",
            Some(event_id),
            Some("remote-node"),
            100,
            100
        )?);

        let records = repository.list_history(10, None)?;
        assert_eq!(records.len(), 1);

        let _ = fs::remove_dir_all(repository.storage.db_root.clone());
        Ok(())
    }

    #[test]
    fn append_with_outbox_enqueue_and_ack_flow() -> Result<(), StorageError> {
        let mut repository = SqliteEventRepository::open(make_config(
            "append-with-outbox",
            LifecycleConfig::default(),
            0,
        ))?;
        repository.init_storage()?;

        let event_id = uuid::Uuid::now_v7();
        let targets = vec![
            " peer-b ".to_string(),
            "peer-a".to_string(),
            "peer-a".to_string(),
        ];

        assert!(repository.append_text_with_outbox(
            "local-text",
            event_id,
            Some("device-local"),
            100,
            100,
            Some(&targets),
            100
        )?);

        let due = repository.list_due_outbox(100, 10)?;
        assert_eq!(due.len(), 2);
        assert!(
            due.iter()
                .all(|message| message.event_id == *event_id.as_bytes())
        );
        assert!(due.iter().all(|message| message.attempt_count == 0));
        let mut due_targets = due
            .iter()
            .filter_map(|message| message.targets.as_ref())
            .filter_map(|targets| targets.first())
            .cloned()
            .collect::<Vec<_>>();
        due_targets.sort();
        assert_eq!(
            due_targets,
            vec!["peer-a".to_string(), "peer-b".to_string()]
        );

        let first_id = due[0].id;
        assert!(repository.try_lease_outbox_message(first_id, 300, 100)?);
        assert!(!repository.try_lease_outbox_message(first_id, 400, 101)?);

        for message in due {
            assert!(repository.mark_outbox_sent(message.id, 120)?);
        }
        let after_ack = repository.list_due_outbox(1_000, 10)?;
        assert!(after_ack.is_empty());

        let _ = fs::remove_dir_all(repository.storage.db_root.clone());
        Ok(())
    }

    #[test]
    fn outbox_retry_updates_attempt_count_and_next_attempt() -> Result<(), StorageError> {
        let mut repository = SqliteEventRepository::open(make_config(
            "outbox-retry",
            LifecycleConfig::default(),
            0,
        ))?;
        repository.init_storage()?;

        let event_id = uuid::Uuid::now_v7();
        assert!(repository.append_text_with_outbox(
            "retry-text",
            event_id,
            Some("device-local"),
            10,
            10,
            None,
            10
        )?);

        let due = repository.list_due_outbox(10, 10)?;
        assert_eq!(due.len(), 1);
        let id = due[0].id;
        assert!(repository.try_lease_outbox_message(id, 100, 10)?);

        assert!(repository.mark_outbox_retry(id, 210, "network down", 20)?);
        let not_due = repository.list_due_outbox(200, 10)?;
        assert!(not_due.is_empty());

        let due_again = repository.list_due_outbox(210, 10)?;
        assert_eq!(due_again.len(), 1);
        assert_eq!(due_again[0].attempt_count, 1);
        assert_eq!(due_again[0].next_attempt_at_ms, 210);
        assert!(due_again[0].targets.is_none());

        let _ = fs::remove_dir_all(repository.storage.db_root.clone());
        Ok(())
    }
}
