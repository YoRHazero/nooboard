use std::collections::HashSet;

use super::defaults::APP_CONFIG_VERSION;
use super::schema::AppConfig;
use crate::{AppError, AppResult};

impl AppConfig {
    pub fn validate(&self) -> AppResult<()> {
        if self.meta.config_version != APP_CONFIG_VERSION {
            return Err(AppError::InvalidConfig(format!(
                "meta.config_version must be {APP_CONFIG_VERSION}, got {}",
                self.meta.config_version
            )));
        }

        if self.identity.device_id.trim().is_empty() {
            return Err(AppError::InvalidConfig(
                "identity.device_id must not be empty".to_string(),
            ));
        }

        if self.app.clipboard.recent_event_lookup_limit == 0 {
            return Err(AppError::InvalidConfig(
                "app.clipboard.recent_event_lookup_limit must be > 0".to_string(),
            ));
        }
        if self.storage.max_text_bytes == 0 {
            return Err(AppError::InvalidConfig(
                "storage.max_text_bytes must be > 0".to_string(),
            ));
        }

        if self.storage.lifecycle.history_window_days < 1 {
            return Err(AppError::InvalidConfig(
                "storage.lifecycle.history_window_days must be >= 1".to_string(),
            ));
        }
        if self.storage.lifecycle.dedup_window_days < self.storage.lifecycle.history_window_days {
            return Err(AppError::InvalidConfig(
                "storage.lifecycle.dedup_window_days must be >= history_window_days".to_string(),
            ));
        }
        if self.storage.lifecycle.gc_every_inserts < 1 {
            return Err(AppError::InvalidConfig(
                "storage.lifecycle.gc_every_inserts must be >= 1".to_string(),
            ));
        }
        if self.storage.lifecycle.gc_batch_size < 1 {
            return Err(AppError::InvalidConfig(
                "storage.lifecycle.gc_batch_size must be >= 1".to_string(),
            ));
        }

        let mut manual_peers = HashSet::new();
        for peer in &self.sync.network.manual_peers {
            if !manual_peers.insert(*peer) {
                return Err(AppError::InvalidConfig(format!(
                    "sync.network.manual_peers contains duplicate address {peer}"
                )));
            }
        }

        let _ = self.to_sync_config()?;
        Ok(())
    }
}
