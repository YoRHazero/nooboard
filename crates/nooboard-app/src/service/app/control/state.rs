use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::clipboard_runtime::ClipboardRuntime;
use crate::config::AppConfig;
use crate::service::events::SubscriptionHub;
use crate::service::types::{
    AppServiceSnapshot, ConnectedPeer, NoobId, StorageConfigView, SyncDesiredState,
};
use crate::storage_runtime::StorageRuntime;
use crate::sync_runtime::SyncRuntime;

pub(crate) struct ControlState {
    pub(super) config_path: PathBuf,
    pub(super) config: AppConfig,
    pub(super) desired_state: SyncDesiredState,
    pub(super) storage_runtime: Arc<StorageRuntime>,
    pub(super) clipboard: ClipboardRuntime,
    pub(super) sync_runtime: SyncRuntime,
    pub(super) subscriptions: Arc<SubscriptionHub>,
}

impl ControlState {
    pub(crate) fn new(
        config_path: PathBuf,
        config: AppConfig,
        storage_runtime: Arc<StorageRuntime>,
        clipboard: ClipboardRuntime,
        sync_runtime: SyncRuntime,
        subscriptions: Arc<SubscriptionHub>,
    ) -> Self {
        Self {
            config_path,
            config,
            desired_state: SyncDesiredState::Stopped,
            storage_runtime,
            clipboard,
            sync_runtime,
            subscriptions,
        }
    }

    pub(super) fn snapshot(&self) -> AppServiceSnapshot {
        AppServiceSnapshot {
            local_noob_id: NoobId::new(self.config.noob_id().unwrap_or_default().to_string()),
            desired_state: self.desired_state,
            actual_sync_status: self.sync_runtime.status().into(),
            connected_peers: self
                .sync_runtime
                .connected_peers()
                .into_iter()
                .map(ConnectedPeer::from)
                .collect(),
            network_enabled: self.config.sync.network.enabled,
            mdns_enabled: self.config.sync.network.mdns_enabled,
            manual_peers: self.config.sync.network.manual_peers.clone(),
            storage: self.storage_view(),
        }
    }

    pub(super) fn config_base_dir(&self) -> PathBuf {
        self.config_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf()
    }

    fn storage_view(&self) -> StorageConfigView {
        StorageConfigView {
            db_root: self.config.storage.db_root.clone(),
            retain_old_versions: self.config.storage.retain_old_versions,
            history_window_days: self.config.storage.lifecycle.history_window_days,
            dedup_window_days: self.config.storage.lifecycle.dedup_window_days,
            gc_every_inserts: self.config.storage.lifecycle.gc_every_inserts,
            gc_batch_size: self.config.storage.lifecycle.gc_batch_size,
        }
    }
}
