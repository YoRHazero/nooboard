use std::path::PathBuf;

#[derive(Clone, PartialEq, Eq)]
pub(super) struct StorageConfigSignature {
    db_root: PathBuf,
    retain_old_versions: usize,
    history_window_days: u32,
    dedup_window_days: u32,
    gc_every_inserts: u32,
    gc_batch_size: u32,
}

impl StorageConfigSignature {
    pub(super) fn from_config(config: &nooboard_storage::AppConfig) -> Self {
        Self {
            db_root: config.storage.db_root.clone(),
            retain_old_versions: config.storage.retain_old_versions,
            history_window_days: config.storage.lifecycle.history_window_days,
            dedup_window_days: config.storage.lifecycle.dedup_window_days,
            gc_every_inserts: config.storage.lifecycle.gc_every_inserts,
            gc_batch_size: config.storage.lifecycle.gc_batch_size,
        }
    }
}
