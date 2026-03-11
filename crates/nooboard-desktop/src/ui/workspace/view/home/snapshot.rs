use nooboard_app::{ClipboardRecordSource, ConnectedPeer, EventId, SyncActualStatus};

use crate::state::live_app::{LiveAppStore, RecentActivityItem};
use crate::ui::workspace::view::shared::clock_label_from_millis;

#[derive(Clone)]
pub(super) struct HomeSnapshot {
    pub system_core: HomeSystemCoreSnapshot,
    pub recent_activity: Vec<RecentActivityItem>,
}

#[derive(Clone)]
pub(super) struct HomeSystemCoreSnapshot {
    pub local_device_id: String,
    pub network_control: HomeNetworkControlSnapshot,
    pub auto_adopt_remote_clipboard: bool,
    pub radar: HomeRadarSnapshot,
    pub clipboard: HomeClipboardSnapshot,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct HomeNetworkControlSnapshot {
    pub desired_running: bool,
    pub disabled: bool,
}

#[derive(Clone)]
pub(super) struct HomeRadarSnapshot {
    pub state: HomeRadarVisualState,
    pub peers: Vec<HomeRadarPeerSnapshot>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum HomeRadarVisualState {
    Running,
    Starting,
    Stopped,
    Disabled,
    Error,
}

#[derive(Clone)]
pub(super) struct HomeRadarPeerSnapshot {
    pub noob_id: String,
    pub address_label: String,
    pub transport_label: String,
    pub visual_state: HomeRadarPeerVisualState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum HomeRadarPeerVisualState {
    Connected,
    Transferring,
}

#[derive(Clone)]
pub(super) struct HomeClipboardSnapshot {
    pub latest_record: Option<HomeClipboardRecordSnapshot>,
    pub adopt_event_id: Option<EventId>,
}

#[derive(Clone)]
pub(super) struct HomeClipboardRecordSnapshot {
    pub source: ClipboardRecordSource,
    pub device_label: String,
    pub recorded_at_label: String,
    pub content: String,
}

impl HomeRadarVisualState {
    pub fn scans(self) -> bool {
        matches!(self, Self::Running | Self::Starting)
    }
}

pub(super) fn build_home_snapshot(store: &LiveAppStore) -> HomeSnapshot {
    let app_state = store.app_state();
    let recent_activity = store.recent_activity().iter().take(5).cloned().collect();
    let transferring_peers: std::collections::BTreeSet<_> = app_state
        .transfers
        .active
        .iter()
        .map(|transfer| transfer.peer_noob_id.clone())
        .collect();

    HomeSnapshot {
        system_core: HomeSystemCoreSnapshot {
            local_device_id: app_state.identity.device_id.clone(),
            network_control: HomeNetworkControlSnapshot {
                desired_running: matches!(
                    app_state.sync.desired,
                    nooboard_app::SyncDesiredState::Running
                ),
                disabled: !app_state.settings.network.network_enabled,
            },
            auto_adopt_remote_clipboard: store.local_preferences().auto_adopt_remote_clipboard,
            radar: HomeRadarSnapshot {
                state: radar_state(&app_state.sync.actual),
                peers: app_state
                    .peers
                    .connected
                    .iter()
                    .map(|peer| {
                        radar_peer_snapshot(peer, transferring_peers.contains(&peer.noob_id))
                    })
                    .collect(),
            },
            clipboard: clipboard_snapshot(store),
        },
        recent_activity,
    }
}

fn radar_state(actual: &SyncActualStatus) -> HomeRadarVisualState {
    match actual {
        SyncActualStatus::Running => HomeRadarVisualState::Running,
        SyncActualStatus::Starting => HomeRadarVisualState::Starting,
        SyncActualStatus::Stopped => HomeRadarVisualState::Stopped,
        SyncActualStatus::Disabled => HomeRadarVisualState::Disabled,
        SyncActualStatus::Error(_) => HomeRadarVisualState::Error,
    }
}

fn radar_peer_snapshot(peer: &ConnectedPeer, transferring: bool) -> HomeRadarPeerSnapshot {
    let address_label = peer
        .addresses
        .first()
        .map(|addr| addr.to_string())
        .unwrap_or_else(|| "unknown address".to_string());
    let transport_label = format!("{:?}", peer.transport);

    HomeRadarPeerSnapshot {
        noob_id: peer.noob_id.as_str().to_string(),
        address_label,
        transport_label,
        visual_state: if transferring {
            HomeRadarPeerVisualState::Transferring
        } else {
            HomeRadarPeerVisualState::Connected
        },
    }
}

fn clipboard_snapshot(store: &LiveAppStore) -> HomeClipboardSnapshot {
    let app_state = store.app_state();
    let Some(record) = store.latest_committed_record() else {
        return HomeClipboardSnapshot {
            latest_record: None,
            adopt_event_id: None,
        };
    };

    let latest_record = HomeClipboardRecordSnapshot {
        source: record.source.clone(),
        device_label: record.origin_device_id.clone(),
        recorded_at_label: clock_label_from_millis(record.created_at_ms),
        content: record.content.clone(),
    };

    let manual_adopt_remote = matches!(record.source, ClipboardRecordSource::RemoteSync)
        && !store.local_preferences().auto_adopt_remote_clipboard
        && record.origin_noob_id != app_state.identity.noob_id;

    HomeClipboardSnapshot {
        latest_record: Some(latest_record),
        adopt_event_id: manual_adopt_remote.then_some(record.event_id),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use nooboard_app::{
        AppState, ClipboardRecord, ClipboardSettings, ClipboardState, EventId, LocalIdentity,
        NetworkSettings, NoobId, PeerTransport, PeersState, SettingsState, StorageSettings,
        SyncActualStatus, SyncDesiredState, SyncState, Transfer, TransferDirection, TransferId,
        TransferSettings, TransferState, TransfersState,
    };

    use crate::state::live_app::LiveAppStore;

    use super::*;

    #[test]
    fn disabled_network_maps_to_disabled_radar_and_stopped_control() {
        let mut store = sample_store();
        store.apply_state_snapshot(AppState {
            sync: SyncState {
                desired: SyncDesiredState::Stopped,
                actual: SyncActualStatus::Disabled,
            },
            settings: SettingsState {
                network: NetworkSettings {
                    listen_port: 17890,
                    network_enabled: false,
                    mdns_enabled: true,
                    manual_peers: Vec::new(),
                },
                ..store.app_state().settings.clone()
            },
            ..store.app_state().clone()
        });

        let snapshot = build_home_snapshot(&store);
        assert_eq!(
            snapshot.system_core.radar.state,
            HomeRadarVisualState::Disabled
        );
        assert!(!snapshot.system_core.network_control.desired_running);
        assert!(snapshot.system_core.network_control.disabled);
    }

    #[test]
    fn remote_clipboard_record_requires_manual_adopt_when_pref_off() {
        let mut store = sample_store();
        store.replace_latest_committed_record(Some(ClipboardRecord {
            event_id: EventId::new(),
            source: ClipboardRecordSource::RemoteSync,
            origin_noob_id: NoobId::new("remote-node"),
            origin_device_id: "remote-mbp".to_string(),
            created_at_ms: 10_000,
            applied_at_ms: 10_000,
            content: "remote text".to_string(),
        }));
        store.apply_state_snapshot(AppState {
            clipboard: ClipboardState {
                latest_committed_event_id: store
                    .latest_committed_record()
                    .map(|record| record.event_id),
            },
            ..store.app_state().clone()
        });

        let snapshot = build_home_snapshot(&store);
        assert!(snapshot.system_core.clipboard.adopt_event_id.is_some());
    }

    fn sample_store() -> LiveAppStore {
        let app_state = AppState {
            revision: 1,
            identity: LocalIdentity {
                noob_id: NoobId::new("local-node"),
                device_id: "desk-01".to_string(),
            },
            local_connection: nooboard_app::LocalConnectionInfo {
                device_endpoint: Some("192.168.1.50:17890".parse().unwrap()),
            },
            sync: SyncState {
                desired: SyncDesiredState::Running,
                actual: SyncActualStatus::Running,
            },
            peers: PeersState {
                connected: vec![nooboard_app::ConnectedPeer {
                    noob_id: NoobId::new("remote-node"),
                    device_id: "remote-mbp".to_string(),
                    addresses: vec!["127.0.0.1:17890".parse().unwrap()],
                    transport: PeerTransport::Manual,
                    latency_ms: Some(22),
                }],
            },
            clipboard: ClipboardState::default(),
            transfers: TransfersState {
                incoming_pending: Vec::new(),
                active: vec![Transfer {
                    transfer_id: TransferId::new(NoobId::new("remote-node"), 1),
                    direction: TransferDirection::Upload,
                    peer_noob_id: NoobId::new("remote-node"),
                    peer_device_id: "remote-mbp".to_string(),
                    file_name: "demo.txt".to_string(),
                    file_size: 10,
                    transferred_bytes: 5,
                    state: TransferState::InProgress,
                    started_at_ms: 0,
                    updated_at_ms: 0,
                }],
                recent_completed: Vec::new(),
            },
            settings: SettingsState {
                identity: nooboard_app::IdentitySettings {
                    device_id: "desk-01".to_string(),
                },
                network: NetworkSettings {
                    listen_port: 17890,
                    network_enabled: true,
                    mdns_enabled: true,
                    manual_peers: Vec::new(),
                },
                storage: StorageSettings {
                    db_root: PathBuf::from("."),
                    history_window_days: 7,
                    dedup_window_days: 7,
                    max_text_bytes: 4096,
                    gc_batch_size: 64,
                },
                clipboard: ClipboardSettings {
                    local_capture_enabled: true,
                },
                transfers: TransferSettings {
                    download_dir: PathBuf::from("./downloads"),
                },
            },
        };

        LiveAppStore::new(PathBuf::from("config.toml"), app_state, None)
    }
}
