use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use nooboard_sync::ConnectedPeerInfo;

use crate::AppResult;
use crate::clipboard_runtime::ClipboardRuntime;
use crate::config::AppConfig;
use crate::service::events::EventHub;
use crate::service::mappers::map_connected_peer;
use crate::service::state::StateHub;
use crate::service::types::{
    AppEvent, AppState, ClipboardSettings, ClipboardState, ConnectedPeer, EventId, LocalIdentity,
    NetworkSettings, NoobId, PeerTransport, PeersState, SettingsState, StorageSettings,
    SyncActualStatus, SyncDesiredState, SyncState, TransferSettings, TransfersState,
};
use crate::storage_runtime::StorageRuntime;
use crate::sync_runtime::SyncRuntime;

const RECENT_COMPLETED_LIMIT: usize = 64;

pub(crate) struct ControlState {
    pub(super) config_path: PathBuf,
    pub(super) config: AppConfig,
    pub(super) storage_runtime: Arc<StorageRuntime>,
    pub(super) clipboard: ClipboardRuntime,
    pub(super) sync_runtime: SyncRuntime,
    pub(super) state_hub: StateHub,
    pub(super) event_hub: EventHub,
    pub(super) app_state: AppState,
}

impl ControlState {
    pub(crate) fn new(
        config_path: PathBuf,
        config: AppConfig,
        storage_runtime: Arc<StorageRuntime>,
        clipboard: ClipboardRuntime,
        sync_runtime: SyncRuntime,
        state_hub: Option<StateHub>,
        event_hub: Option<EventHub>,
    ) -> AppResult<Self> {
        let identity = LocalIdentity {
            noob_id: NoobId::new(config.noob_id().unwrap_or_default().to_string()),
            device_id: config.identity.device_id.clone(),
        };
        let peers = map_connected_peers(&config, sync_runtime.connected_peers());
        let app_state = AppState {
            revision: 0,
            identity,
            sync: SyncState {
                desired: SyncDesiredState::Stopped,
                actual: SyncActualStatus::from(sync_runtime.status()),
            },
            peers: PeersState {
                connected: peers.clone(),
            },
            clipboard: ClipboardState {
                latest_committed_event_id: load_initial_latest_committed_event_id(&config)?,
            },
            transfers: TransfersState::default(),
            settings: settings_state(&config),
        };
        let state_hub = state_hub.unwrap_or_else(|| StateHub::new(app_state.clone()));
        let event_hub = event_hub.unwrap_or_else(EventHub::new);

        Ok(Self {
            config_path,
            config,
            storage_runtime,
            clipboard,
            sync_runtime,
            state_hub,
            event_hub,
            app_state,
        })
    }

    pub(super) fn get_state(&self) -> AppState {
        self.app_state.clone()
    }

    pub(super) fn publish_event(&self, event: AppEvent) {
        self.event_hub.publish(event);
    }

    pub(super) fn update_state(&mut self, updater: impl FnOnce(&mut AppState)) -> bool {
        let mut next = self.app_state.clone();
        let current_revision = next.revision;
        updater(&mut next);
        next.revision = current_revision;
        if next == self.app_state {
            return false;
        }

        next.revision = current_revision.saturating_add(1);
        self.app_state = next;
        self.state_hub.publish(self.app_state.clone());
        true
    }

    pub(super) fn config_base_dir(&self) -> PathBuf {
        self.config_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf()
    }

    pub(super) fn sync_actual_status(&self) -> SyncActualStatus {
        self.sync_runtime.status().into()
    }

    pub(super) fn connected_peers_state(&self) -> Vec<ConnectedPeer> {
        map_connected_peers(&self.config, self.sync_runtime.connected_peers())
    }

    pub(super) fn refresh_connected_peers(&mut self, peers: Vec<ConnectedPeer>) {
        self.update_state(|state| {
            state.peers.connected = peers;
        });
    }

    pub(super) fn recent_completed_limit(&self) -> usize {
        RECENT_COMPLETED_LIMIT
    }
}

fn map_connected_peers(config: &AppConfig, peers: Vec<ConnectedPeerInfo>) -> Vec<ConnectedPeer> {
    peers
        .into_iter()
        .map(|peer| {
            let transport = peer_transport(config, peer.addr);
            map_connected_peer(peer, transport)
        })
        .collect()
}

fn peer_transport(config: &AppConfig, addr: SocketAddr) -> PeerTransport {
    let manual = config.sync.network.manual_peers.contains(&addr);
    match (manual, config.sync.network.mdns_enabled) {
        (true, true) => PeerTransport::Mixed,
        (true, false) => PeerTransport::Manual,
        (false, true) => PeerTransport::Mdns,
        (false, false) => PeerTransport::Unknown,
    }
}

pub(super) fn settings_state(config: &AppConfig) -> SettingsState {
    SettingsState {
        network: NetworkSettings {
            network_enabled: config.sync.network.enabled,
            mdns_enabled: config.sync.network.mdns_enabled,
            manual_peers: config.sync.network.manual_peers.clone(),
        },
        storage: StorageSettings {
            db_root: config.storage.db_root.clone(),
            history_window_days: config.storage.lifecycle.history_window_days,
            dedup_window_days: config.storage.lifecycle.dedup_window_days,
            max_text_bytes: config.storage.max_text_bytes,
            gc_batch_size: config.storage.lifecycle.gc_batch_size as usize,
        },
        clipboard: ClipboardSettings {
            local_capture_enabled: config.local_capture_enabled(),
        },
        transfers: TransferSettings {
            download_dir: config.sync.file.download_dir.clone(),
        },
    }
}

fn load_initial_latest_committed_event_id(config: &AppConfig) -> AppResult<Option<EventId>> {
    let mut repository = nooboard_storage::SqliteEventRepository::open(config.to_storage_config())?;
    repository.init_storage()?;
    Ok(repository
        .list_history(1, None)?
        .into_iter()
        .next()
        .map(|record| EventId::from(uuid::Uuid::from_bytes(record.event_id))))
}
