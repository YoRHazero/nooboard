mod clipboard;
mod home;
mod network;
mod settings;
mod shared;
mod transfers;

use nooboard_core::{ClipboardRecord, WorkspaceSnapshot};

pub use clipboard::ClipboardWorkspaceViewState;
#[cfg(test)]
pub use home::{
    HomeClipboardControlViewState, HomeClipboardPanelViewState, HomeNetworkControlViewState,
    HomeRadarViewState,
};
pub use home::{
    HomeClipboardRecordViewState, HomePageViewState, HomeRadarPeerViewState, HomeRadarVisualState,
    HomeSystemCoreViewState,
};
pub use network::{
    NetworkDirectSeedViewState, NetworkLanPeerViewState, NetworkPageViewState,
    NetworkPendingRequestViewState, NetworkSessionViewState,
};
pub use settings::{
    SettingsClipboardViewState, SettingsConnectionViewState, SettingsPageViewState,
    SettingsStorageViewState, SettingsTransfersViewState,
};
pub use shared::WorkspaceSessionTargetViewState;
pub use transfers::{
    ActiveTransferViewState, CompletedTransferViewState, IncomingTransferViewState,
    TransfersPageViewState,
};

#[derive(Clone)]
pub struct WorkspaceIdentityViewState {
    pub headline: String,
    pub subheadline: String,
}

#[derive(Clone)]
pub struct WorkspaceShellMetricsViewState {
    pub session_count_label: String,
    pub inbox_count_label: String,
}

#[derive(Clone)]
pub struct WorkspaceViewState {
    pub identity: WorkspaceIdentityViewState,
    pub shell_metrics: WorkspaceShellMetricsViewState,
    pub home: HomePageViewState,
    pub clipboard: ClipboardWorkspaceViewState,
    pub network: NetworkPageViewState,
    pub transfers: TransfersPageViewState,
    pub settings: SettingsPageViewState,
}

pub fn build_workspace_view_state(
    snapshot: &WorkspaceSnapshot,
    latest_record: Option<&ClipboardRecord>,
) -> WorkspaceViewState {
    WorkspaceViewState {
        identity: WorkspaceIdentityViewState {
            headline: snapshot.identity.device_id.clone(),
            subheadline: snapshot.identity.noob_id.to_string(),
        },
        shell_metrics: WorkspaceShellMetricsViewState {
            session_count_label: snapshot.network.sessions.len().to_string(),
            inbox_count_label: snapshot
                .network
                .transfers
                .incoming_pending
                .len()
                .to_string(),
        },
        home: home::build_home_page_view_state(snapshot, latest_record),
        clipboard: clipboard::build_clipboard_workspace_view_state(snapshot, latest_record),
        network: network::build_network_page_view_state(snapshot),
        transfers: transfers::build_transfers_page_view_state(snapshot),
        settings: settings::build_settings_page_view_state(snapshot),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use nooboard_core::{
        ClipboardRecord, ClipboardRecordSource, ClipboardSettings, ClipboardState,
        ConnectionSettings, EventId, LocalConnectionInfo, NetworkSettings, NetworkSnapshot,
        NetworkStatus, NoobId, StorageSettings, TransferSettings, TransfersSnapshot,
        WorkspaceIdentity, WorkspaceSettings, WorkspaceSnapshot,
    };

    use super::*;

    #[test]
    fn build_workspace_view_state_projects_snapshot_details() {
        let snapshot = sample_snapshot();
        let record = ClipboardRecord {
            event_id: EventId::new(),
            source: ClipboardRecordSource::UserSubmit,
            origin_noob_id: NoobId::new("local-node"),
            origin_device_id: "desk-01".to_string(),
            created_at_ms: 0,
            applied_at_ms: 0,
            content: "hello from clipboard".to_string(),
        };

        let state = build_workspace_view_state(&snapshot, Some(&record));

        assert_eq!(state.identity.headline, "desk-01");
        assert!(state.network.network_enabled);
        assert_eq!(state.home.system_core.local_device_id, "desk-01");
        assert_eq!(
            state.home.system_core.clipboard_control.adopt_event_id,
            Some(record.event_id)
        );
        assert!(matches!(
            state
                .home
                .system_core
                .clipboard
                .latest_record
                .as_ref()
                .map(|item| item.source),
            Some(ClipboardRecordSource::UserSubmit)
        ));
        assert_eq!(state.clipboard.max_text_bytes, 4096);
        assert_eq!(state.clipboard.session_targets.len(), 0);
        assert_eq!(state.settings.connection.device_id, "desk-01");
        assert_eq!(state.settings.connection.listen_port, 17890);
        assert_eq!(
            state.settings.transfers.download_dir,
            PathBuf::from("/tmp/downloads")
        );
    }

    #[test]
    fn build_workspace_view_state_handles_missing_clipboard_record() {
        let snapshot = sample_snapshot();

        let state = build_workspace_view_state(&snapshot, None);

        assert!(state.home.system_core.clipboard.latest_record.is_none());
        assert!(state.clipboard.latest_record.is_none());
    }

    fn sample_snapshot() -> WorkspaceSnapshot {
        WorkspaceSnapshot {
            revision: 42,
            identity: WorkspaceIdentity {
                noob_id: NoobId::new("local-node"),
                device_id: "desk-01".to_string(),
            },
            local_connection: LocalConnectionInfo {
                device_endpoint: Some("127.0.0.1:17890".parse().unwrap()),
            },
            clipboard: ClipboardState::default(),
            settings: WorkspaceSettings {
                connection: ConnectionSettings {
                    device_id: "desk-01".to_string(),
                    token: "shared-token".to_string(),
                },
                network: NetworkSettings {
                    listen_port: 17890,
                    lan_enabled: true,
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
            network: NetworkSnapshot {
                status: NetworkStatus::Running,
                lan_enabled: true,
                lan_peers: Vec::new(),
                direct_seeds: Vec::new(),
                pending_direct_requests: Vec::new(),
                sessions: Vec::new(),
                transfers: TransfersSnapshot::default(),
            },
        }
    }
}
