use std::net::SocketAddr;
use std::path::PathBuf;

use crate::state::live_app::LiveAppStore;

#[derive(Clone, PartialEq, Eq)]
pub(in crate::ui::workspace::view) struct SettingsSnapshot {
    pub identity: IdentitySettingsValue,
    pub network: NetworkSettingsValue,
    pub local_connection: LocalConnectionInfoValue,
    pub storage: StorageSettingsValue,
    pub clipboard: ClipboardSettingsValue,
    pub transfers: TransferSettingsValue,
}

#[derive(Clone, PartialEq, Eq)]
pub(in crate::ui::workspace::view) struct IdentitySettingsValue {
    pub device_id: String,
}

#[derive(Clone, PartialEq, Eq)]
pub(in crate::ui::workspace::view) struct NetworkSettingsValue {
    pub listen_port: u16,
    pub network_enabled: bool,
    pub mdns_enabled: bool,
    pub manual_peers: Vec<SocketAddr>,
}

#[derive(Clone, PartialEq, Eq)]
pub(in crate::ui::workspace::view) struct LocalConnectionInfoValue {
    pub device_endpoint: Option<SocketAddr>,
}

#[derive(Clone, PartialEq, Eq)]
pub(in crate::ui::workspace::view) struct NetworkPanelValue {
    pub device_id: String,
    pub listen_port_text: String,
    pub network_enabled: bool,
    pub mdns_enabled: bool,
    pub manual_peers: Vec<SocketAddr>,
}

impl NetworkPanelValue {
    pub(super) fn from_snapshot(snapshot: &SettingsSnapshot) -> Self {
        Self {
            device_id: snapshot.identity.device_id.clone(),
            listen_port_text: snapshot.network.listen_port.to_string(),
            network_enabled: snapshot.network.network_enabled,
            mdns_enabled: snapshot.network.mdns_enabled,
            manual_peers: snapshot.network.manual_peers.clone(),
        }
    }
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
    let app_state = store.app_state();
    let settings = &app_state.settings;

    SettingsSnapshot {
        identity: IdentitySettingsValue {
            device_id: settings.identity.device_id.clone(),
        },
        network: NetworkSettingsValue {
            listen_port: settings.network.listen_port,
            network_enabled: settings.network.network_enabled,
            mdns_enabled: settings.network.mdns_enabled,
            manual_peers: settings.network.manual_peers.clone(),
        },
        local_connection: LocalConnectionInfoValue {
            device_endpoint: app_state.local_connection.device_endpoint,
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
        AppState, ClipboardSettings, ClipboardState, IdentitySettings, LocalConnectionInfo,
        LocalIdentity, NetworkSettings, NoobId, PeersState, SettingsState, StorageSettings,
        SyncActualStatus, SyncDesiredState, SyncState, TransferSettings, TransfersState,
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
            local_connection: LocalConnectionInfo {
                device_endpoint: Some("192.168.1.50:17890".parse().unwrap()),
            },
            sync: SyncState {
                desired: SyncDesiredState::Stopped,
                actual: SyncActualStatus::Stopped,
            },
            peers: PeersState::default(),
            clipboard: ClipboardState::default(),
            transfers: TransfersState::default(),
            settings: SettingsState {
                identity: IdentitySettings {
                    device_id: "desk-01".to_string(),
                },
                network: NetworkSettings {
                    listen_port: 17890,
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

        assert_eq!(snapshot.identity.device_id, "desk-01");
        assert_eq!(snapshot.network.listen_port, 17890);
        assert!(snapshot.network.network_enabled);
        assert!(!snapshot.network.mdns_enabled);
        assert_eq!(snapshot.network.manual_peers.len(), 1);
        assert_eq!(
            snapshot.local_connection.device_endpoint,
            Some("192.168.1.50:17890".parse().unwrap())
        );
        assert_eq!(snapshot.storage.max_text_bytes, 4096);
        assert!(snapshot.clipboard.local_capture_enabled);
        assert_eq!(
            snapshot.transfers.download_dir,
            PathBuf::from("/tmp/downloads")
        );
    }

    #[test]
    fn network_panel_value_combines_identity_and_network_settings() {
        let snapshot = SettingsSnapshot {
            identity: IdentitySettingsValue {
                device_id: "desk-02".to_string(),
            },
            network: NetworkSettingsValue {
                listen_port: 24001,
                network_enabled: true,
                mdns_enabled: true,
                manual_peers: vec!["127.0.0.1:24002".parse().unwrap()],
            },
            local_connection: LocalConnectionInfoValue {
                device_endpoint: Some("192.168.1.80:24001".parse().unwrap()),
            },
            storage: StorageSettingsValue {
                db_root: PathBuf::from("/tmp/db"),
                history_window_days: 7,
                dedup_window_days: 14,
                max_text_bytes: 1024,
                gc_batch_size: 32,
            },
            clipboard: ClipboardSettingsValue {
                local_capture_enabled: true,
            },
            transfers: TransferSettingsValue {
                download_dir: PathBuf::from("/tmp/downloads"),
            },
        };

        let panel = NetworkPanelValue::from_snapshot(&snapshot);

        assert_eq!(panel.device_id, "desk-02");
        assert_eq!(panel.listen_port_text, "24001");
        assert_eq!(panel.manual_peers.len(), 1);
    }
}
