use super::{AppServiceImpl, StorageConfigView, StoragePatch};
use crate::AppResult;

impl AppServiceImpl {
    pub(super) async fn apply_storage_patch_usecase(
        &self,
        patch: StoragePatch,
    ) -> AppResult<StorageConfigView> {
        let StoragePatch {
            db_root,
            retain_old_versions,
            history_window_days,
            dedup_window_days,
            gc_every_inserts,
            gc_batch_size,
        } = patch;

        let applied = self
            .execute_storage_config_transcation(move |config| {
                if let Some(db_root) = db_root {
                    config.storage.db_root = db_root;
                }
                if let Some(retain_old_versions) = retain_old_versions {
                    config.storage.retain_old_versions = retain_old_versions;
                }
                if let Some(history_window_days) = history_window_days {
                    config.storage.lifecycle.history_window_days = history_window_days;
                }
                if let Some(dedup_window_days) = dedup_window_days {
                    config.storage.lifecycle.dedup_window_days = dedup_window_days;
                }
                if let Some(gc_every_inserts) = gc_every_inserts {
                    config.storage.lifecycle.gc_every_inserts = gc_every_inserts;
                }
                if let Some(gc_batch_size) = gc_batch_size {
                    config.storage.lifecycle.gc_batch_size = gc_batch_size;
                }
                Ok(())
            })
            .await?;

        Ok(StorageConfigView::from_config(&applied))
    }
}

impl StorageConfigView {
    fn from_config(config: &crate::config::AppConfig) -> Self {
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
