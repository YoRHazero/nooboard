use std::fs;
use std::path::Path;

use crate::{StorageConfig, StorageError};

#[derive(Debug, Clone)]
pub struct SqlCatalog {
    pub schema: String,
    pub insert_event: String,
    pub select_latest_active_content: String,
    pub list_history: String,
    pub list_history_with_cursor: String,
    pub search_history: String,
    pub gc_mark_tombstone: String,
    pub gc_delete_expired_tombstone: String,
}

impl SqlCatalog {
    pub fn load(storage: &StorageConfig) -> Result<Self, StorageError> {
        Ok(Self {
            schema: fs::read_to_string(&storage.schema_sql)?,
            insert_event: load_query(&storage.queries_dir, "insert_event.sql")?,
            select_latest_active_content: load_query(
                &storage.queries_dir,
                "select_latest_active_content.sql",
            )?,
            list_history: load_query(&storage.queries_dir, "list_history.sql")?,
            list_history_with_cursor: load_query(
                &storage.queries_dir,
                "list_history_with_cursor.sql",
            )?,
            search_history: load_query(&storage.queries_dir, "search_history.sql")?,
            gc_mark_tombstone: load_query(&storage.queries_dir, "gc_mark_tombstone.sql")?,
            gc_delete_expired_tombstone: load_query(
                &storage.queries_dir,
                "gc_delete_expired_tombstone.sql",
            )?,
        })
    }
}

fn load_query(queries_dir: &Path, file_name: &str) -> Result<String, StorageError> {
    let path = queries_dir.join(file_name);
    fs::read_to_string(path).map_err(Into::into)
}
