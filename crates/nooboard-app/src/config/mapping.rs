use nooboard_sync::SyncConfig;
use nooboard_sync::protocol::PROTOCOL_VERSION;

use super::schema::AppConfig;
use crate::{AppError, AppResult};

impl AppConfig {
    pub fn to_storage_config(&self) -> nooboard_storage::AppConfig {
        nooboard_storage::AppConfig {
            storage: nooboard_storage::StorageConfig {
                db_root: self.storage.db_root.clone(),
                retain_old_versions: self.storage.retain_old_versions,
                lifecycle: nooboard_storage::LifecycleConfig {
                    history_window_days: self.storage.lifecycle.history_window_days,
                    dedup_window_days: self.storage.lifecycle.dedup_window_days,
                    gc_every_inserts: self.storage.lifecycle.gc_every_inserts,
                    gc_batch_size: self.storage.lifecycle.gc_batch_size,
                },
            },
        }
    }

    pub fn to_sync_config(&self) -> AppResult<SyncConfig> {
        let node_id = self.node_id.clone().ok_or_else(|| {
            AppError::InvalidConfig("identity.node_id was not initialized".to_string())
        })?;
        if node_id.trim().is_empty() {
            return Err(AppError::InvalidConfig(
                "identity.noob_id_file produced empty node_id".to_string(),
            ));
        }

        let sync_config = SyncConfig {
            enabled: self.sync.network.enabled,
            mdns_enabled: self.sync.network.mdns_enabled,
            listen_addr: self.sync.network.listen_addr,
            token: self.sync.auth.token.clone(),
            manual_peers: self.sync.network.manual_peers.clone(),
            protocol_version: PROTOCOL_VERSION,
            connect_timeout_ms: self.sync.transport.connect_timeout_ms,
            handshake_timeout_ms: self.sync.transport.handshake_timeout_ms,
            ping_interval_ms: self.sync.transport.ping_interval_ms,
            pong_timeout_ms: self.sync.transport.pong_timeout_ms,
            max_packet_size: self.sync.transport.max_packet_size,
            file_chunk_size: self.sync.file.chunk_size,
            file_decision_timeout_ms: self.sync.file.decision_timeout_ms,
            transfer_idle_timeout_ms: self.sync.file.idle_timeout_ms,
            download_dir: self.sync.file.download_dir.clone(),
            max_file_size: self.sync.file.max_file_size,
            active_downloads: self.sync.file.active_downloads,
            noob_id: node_id,
            device_id: self.identity.device_id.clone(),
        };

        sync_config.validate().map_err(|message| {
            AppError::InvalidConfig(format!("sync config invalid: {message}"))
        })?;
        Ok(sync_config)
    }

    pub fn recent_event_lookup_limit(&self) -> usize {
        self.app.clipboard.recent_event_lookup_limit
    }

    pub fn node_id(&self) -> Option<&str> {
        self.node_id.as_deref()
    }
}
