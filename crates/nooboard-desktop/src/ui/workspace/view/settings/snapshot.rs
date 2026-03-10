use std::net::SocketAddr;
use std::path::PathBuf;

use crate::state::live_app::LiveAppStore;

#[derive(Clone, PartialEq, Eq)]
pub(in crate::ui::workspace::view) struct SettingsSnapshot {
    pub network: NetworkSettingsValue,
    pub storage: StorageSettingsValue,
    pub clipboard: ClipboardSettingsValue,
    pub transfers: TransferSettingsValue,
}

#[derive(Clone, PartialEq, Eq)]
pub(in crate::ui::workspace::view) struct NetworkSettingsValue {
    pub network_enabled: bool,
    pub mdns_enabled: bool,
    pub manual_peers: Vec<SocketAddr>,
}

#[derive(Clone, PartialEq, Eq)]
pub(in crate::ui::workspace::view) struct StorageSettingsValue {
    pub db_root: PathBuf,
    pub history_window_days: u32,
    pub dedup_window_days: u32,
    pub max_text_bytes: usize,
    pub gc_batch_size: usize,
}

#[derive(Clone, PartialEq, Eq)]
pub(in crate::ui::workspace::view) struct ClipboardSettingsValue {
    pub local_capture_enabled: bool,
}

#[derive(Clone, PartialEq, Eq)]
pub(in crate::ui::workspace::view) struct TransferSettingsValue {
    pub download_dir: PathBuf,
}

pub(in crate::ui::workspace::view) fn build_settings_snapshot(
    store: &LiveAppStore,
) -> SettingsSnapshot {
    let settings = &store.app_state().settings;

    SettingsSnapshot {
        network: NetworkSettingsValue {
            network_enabled: settings.network.network_enabled,
            mdns_enabled: settings.network.mdns_enabled,
            manual_peers: settings.network.manual_peers.clone(),
        },
        storage: StorageSettingsValue {
            db_root: settings.storage.db_root.clone(),
            history_window_days: settings.storage.history_window_days,
            dedup_window_days: settings.storage.dedup_window_days,
            max_text_bytes: settings.storage.max_text_bytes,
            gc_batch_size: settings.storage.gc_batch_size,
        },
        clipboard: ClipboardSettingsValue {
            local_capture_enabled: settings.clipboard.local_capture_enabled,
        },
        transfers: TransferSettingsValue {
            download_dir: settings.transfers.download_dir.clone(),
        },
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use nooboard_app::{
        AppState, ClipboardSettings, ClipboardState, LocalIdentity, NetworkSettings, NoobId,
        PeersState, SettingsState, StorageSettings, SyncActualStatus, SyncDesiredState, SyncState,
        TransferSettings, TransfersState,
    };

    use crate::state::live_app::LiveAppStore;

    use super::*;

    #[test]
    fn snapshot_mirrors_current_app_settings() {
        let app_state = AppState {
            revision: 1,
            identity: LocalIdentity {
                noob_id: NoobId::new("local-node"),
                device_id: "desk-01".to_string(),
            },
            sync: SyncState {
                desired: SyncDesiredState::Stopped,
                actual: SyncActualStatus::Stopped,
            },
            peers: PeersState::default(),
            clipboard: ClipboardState::default(),
            transfers: TransfersState::default(),
            settings: SettingsState {
                network: NetworkSettings {
                    network_enabled: true,
                    mdns_enabled: false,
                    manual_peers: vec!["127.0.0.1:24001".parse().unwrap()],
                },
                storage: StorageSettings {
                    db_root: PathBuf::from("/tmp/db"),
                    history_window_days: 7,
                    dedup_window_days: 14,
                    max_text_bytes: 4096,
                    gc_batch_size: 64,
                },
                clipboard: ClipboardSettings {
                    local_capture_enabled: true,
                },
                transfers: TransferSettings {
                    download_dir: PathBuf::from("/tmp/downloads"),
                },
            },
        };
        let store = LiveAppStore::new(PathBuf::from("config.toml"), app_state, None);

        let snapshot = build_settings_snapshot(&store);

        assert!(snapshot.network.network_enabled);
        assert!(!snapshot.network.mdns_enabled);
        assert_eq!(snapshot.network.manual_peers.len(), 1);
        assert_eq!(snapshot.storage.max_text_bytes, 4096);
        assert!(snapshot.clipboard.local_capture_enabled);
        assert_eq!(
            snapshot.transfers.download_dir,
            PathBuf::from("/tmp/downloads")
        );
    }
}
