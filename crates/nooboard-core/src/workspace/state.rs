use std::path::{Path, PathBuf};

use nooboard_config::AppConfig;
use nooboard_network::{NetworkRuntime, NetworkSnapshot};

use crate::clipboard::ClipboardRuntime;
use crate::storage::StorageRuntime;
use crate::types::{
    ClipboardState, ConnectionSettings, EventId, EventSubscription, LocalConnectionInfo,
    NetworkSettings, NoobId, StateSubscription, StorageSettings, TransferSettings, WorkspaceEvent,
    WorkspaceIdentity, WorkspaceSettings, WorkspaceSnapshot,
};

use super::subscriptions::{EventHub, StateHub};

pub(crate) struct WorkspaceState {
    config_path: PathBuf,
    config: AppConfig,
    storage_runtime: StorageRuntime,
    clipboard_runtime: ClipboardRuntime,
    network_runtime: NetworkRuntime,
    local_connection: LocalConnectionInfo,
    latest_committed_event_id: Option<EventId>,
    state_hub: StateHub,
    event_hub: EventHub,
    snapshot: WorkspaceSnapshot,
}

impl WorkspaceState {
    pub(crate) fn new(
        config_path: PathBuf,
        config: AppConfig,
        storage_runtime: StorageRuntime,
        clipboard_runtime: ClipboardRuntime,
        network_runtime: NetworkRuntime,
        local_connection: LocalConnectionInfo,
        latest_committed_event_id: Option<EventId>,
        network_snapshot: NetworkSnapshot,
    ) -> Self {
        let snapshot = WorkspaceSnapshot {
            revision: 0,
            identity: workspace_identity(&config),
            local_connection: local_connection.clone(),
            clipboard: ClipboardState {
                latest_committed_event_id,
            },
            settings: workspace_settings(&config),
            network: network_snapshot,
        };
        let state_hub = StateHub::new(snapshot.clone());
        let event_hub = EventHub::new();

        Self {
            config_path,
            config,
            storage_runtime,
            clipboard_runtime,
            network_runtime,
            local_connection,
            latest_committed_event_id,
            state_hub,
            event_hub,
            snapshot,
        }
    }

    pub(crate) fn config_path(&self) -> &Path {
        &self.config_path
    }

    pub(crate) fn config_base_dir(&self) -> PathBuf {
        self.config_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf()
    }

    pub(crate) fn config(&self) -> &AppConfig {
        &self.config
    }

    pub(crate) fn replace_config(&mut self, config: AppConfig) {
        self.config = config;
    }

    pub(crate) fn storage_runtime(&self) -> &StorageRuntime {
        &self.storage_runtime
    }

    pub(crate) fn clipboard_runtime(&self) -> &ClipboardRuntime {
        &self.clipboard_runtime
    }

    pub(crate) fn network_runtime(&self) -> &NetworkRuntime {
        &self.network_runtime
    }

    pub(crate) fn replace_network_runtime(&mut self, runtime: NetworkRuntime) {
        self.network_runtime = runtime;
    }

    pub(crate) fn current_snapshot(&self) -> &WorkspaceSnapshot {
        &self.snapshot
    }

    pub(crate) fn snapshot(&self) -> WorkspaceSnapshot {
        self.snapshot.clone()
    }

    pub(crate) fn subscribe_state(&self) -> StateSubscription {
        self.state_hub.subscribe()
    }

    pub(crate) fn subscribe_events(&self) -> EventSubscription {
        self.event_hub.subscribe()
    }

    pub(crate) fn publish_event(&self, event: WorkspaceEvent) {
        self.event_hub.publish(event);
    }

    pub(crate) fn set_local_connection(&mut self, local_connection: LocalConnectionInfo) {
        self.local_connection = local_connection;
    }

    pub(crate) fn set_latest_committed_event_id(&mut self, event_id: Option<EventId>) {
        self.latest_committed_event_id = event_id;
    }

    pub(crate) fn refresh_snapshot(&mut self, network_snapshot: NetworkSnapshot) -> bool {
        let current_revision = self.snapshot.revision;
        let mut next = WorkspaceSnapshot {
            revision: current_revision,
            identity: workspace_identity(&self.config),
            local_connection: self.local_connection.clone(),
            clipboard: ClipboardState {
                latest_committed_event_id: self.latest_committed_event_id,
            },
            settings: workspace_settings(&self.config),
            network: network_snapshot,
        };

        if next == self.snapshot {
            return false;
        }

        next.revision = current_revision.saturating_add(1);
        self.snapshot = next;
        self.state_hub.publish(self.snapshot.clone());
        true
    }
}

fn workspace_identity(config: &AppConfig) -> WorkspaceIdentity {
    WorkspaceIdentity {
        noob_id: NoobId::new(config.noob_id().unwrap_or_default().to_string()),
        device_id: config.identity.device_id.clone(),
    }
}

fn workspace_settings(config: &AppConfig) -> WorkspaceSettings {
    WorkspaceSettings {
        connection: ConnectionSettings {
            device_id: config.identity.device_id.clone(),
            token: config.network.auth.token.clone(),
        },
        network: NetworkSettings {
            listen_port: config.network.listen_port,
            lan_enabled: config.network.lan.enabled,
        },
        storage: StorageSettings {
            db_root: config.storage.db_root.clone(),
            history_window_days: config.storage.lifecycle.history_window_days,
            dedup_window_days: config.storage.lifecycle.dedup_window_days,
            max_text_bytes: config.storage.max_text_bytes,
            gc_batch_size: usize::try_from(config.storage.lifecycle.gc_batch_size)
                .unwrap_or(usize::MAX),
        },
        clipboard: crate::ClipboardSettings {
            local_capture_enabled: config.local_capture_enabled(),
        },
        transfers: TransferSettings {
            download_dir: config.network.transfer.download_dir.clone(),
        },
    }
}
