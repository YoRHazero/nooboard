use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension, params};

use crate::{AppConfig, ClipboardRecord, StorageError};

pub trait ClipboardRepository {
    fn init_schema(&self) -> Result<(), StorageError>;
    fn insert_text_event(&self, text: &str, captured_at: i64) -> Result<(), StorageError>;
    fn list_recent(&self, limit: usize) -> Result<Vec<ClipboardRecord>, StorageError>;
}

pub struct SqliteClipboardRepository {
    conn: Connection,
    schema_path: PathBuf,
}

impl SqliteClipboardRepository {
    pub fn open_from_config(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        let config = AppConfig::load(path)?;
        Self::open(&config.storage.db_path, &config.storage.schema_path)
    }

    pub fn open(
        db_path: impl AsRef<Path>,
        schema_path: impl AsRef<Path>,
    ) -> Result<Self, StorageError> {
        let db_path = db_path.as_ref().to_path_buf();
        if let Some(parent_dir) = db_path.parent() {
            fs::create_dir_all(parent_dir)?;
        }

        let conn = Connection::open(db_path)?;
        Ok(Self {
            conn,
            schema_path: schema_path.as_ref().to_path_buf(),
        })
    }
}

impl ClipboardRepository for SqliteClipboardRepository {
    fn init_schema(&self) -> Result<(), StorageError> {
        let schema_sql = fs::read_to_string(&self.schema_path)?;
        self.conn.execute_batch(&schema_sql)?;
        Ok(())
    }

    fn insert_text_event(&self, text: &str, captured_at: i64) -> Result<(), StorageError> {
        let latest_content = self
            .conn
            .query_row(
                r#"
                SELECT content
                FROM clipboard_history
                ORDER BY id DESC
                LIMIT 1
                "#,
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        if latest_content.as_deref() == Some(text) {
            return Ok(());
        }

        self.conn.execute(
            r#"
            INSERT INTO clipboard_history (content, captured_at)
            VALUES (?1, ?2)
            "#,
            params![text, captured_at],
        )?;
        Ok(())
    }

    fn list_recent(&self, limit: usize) -> Result<Vec<ClipboardRecord>, StorageError> {
        if limit == 0 {
            return Ok(Vec::new());
        }

        let limit = i64::try_from(limit).map_err(|_| StorageError::LimitOutOfRange(limit))?;
        let mut statement = self.conn.prepare(
            r#"
            SELECT id, content, captured_at
            FROM clipboard_history
            ORDER BY captured_at DESC, id DESC
            LIMIT ?1
            "#,
        )?;

        let rows = statement.query_map([limit], |row| {
            Ok(ClipboardRecord {
                id: row.get(0)?,
                content: row.get(1)?,
                captured_at: row.get(2)?,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use std::process;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    fn temp_db_path(name: &str) -> PathBuf {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or(0);
        std::env::temp_dir().join(format!(
            "nooboard-storage-{name}-{}-{millis}.db",
            process::id()
        ))
    }

    fn workspace_schema_path() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("sql")
            .join("schema.sql")
    }

    #[test]
    fn init_schema_creates_history_table() -> Result<(), StorageError> {
        let db_path = temp_db_path("schema");
        let schema_path = workspace_schema_path();
        let repository = SqliteClipboardRepository::open(&db_path, &schema_path)?;

        repository.init_schema()?;

        let exists: i64 = repository.conn.query_row(
            r#"
            SELECT COUNT(1)
            FROM sqlite_master
            WHERE type = 'table' AND name = 'clipboard_history'
            "#,
            [],
            |row| row.get(0),
        )?;
        assert_eq!(exists, 1);

        let _ = fs::remove_file(db_path);
        Ok(())
    }

    #[test]
    fn list_recent_returns_descending_records() -> Result<(), StorageError> {
        let db_path = temp_db_path("recent");
        let schema_path = workspace_schema_path();
        let repository = SqliteClipboardRepository::open(&db_path, &schema_path)?;

        repository.init_schema()?;
        repository.insert_text_event("first", 100)?;
        repository.insert_text_event("second", 200)?;
        repository.insert_text_event("third", 300)?;

        let records = repository.list_recent(2)?;
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].content, "third");
        assert_eq!(records[0].captured_at, 300);
        assert_eq!(records[1].content, "second");
        assert_eq!(records[1].captured_at, 200);

        let _ = fs::remove_file(db_path);
        Ok(())
    }

    #[test]
    fn insert_skips_consecutive_duplicate_content() -> Result<(), StorageError> {
        let db_path = temp_db_path("dedup");
        let schema_path = workspace_schema_path();
        let repository = SqliteClipboardRepository::open(&db_path, &schema_path)?;

        repository.init_schema()?;
        repository.insert_text_event("dup", 100)?;
        repository.insert_text_event("dup", 200)?;
        repository.insert_text_event("other", 300)?;
        repository.insert_text_event("dup", 400)?;

        let records = repository.list_recent(10)?;
        assert_eq!(records.len(), 3);
        assert_eq!(records[0].content, "dup");
        assert_eq!(records[0].captured_at, 400);
        assert_eq!(records[1].content, "other");
        assert_eq!(records[1].captured_at, 300);
        assert_eq!(records[2].content, "dup");
        assert_eq!(records[2].captured_at, 100);

        let _ = fs::remove_file(db_path);
        Ok(())
    }
}
