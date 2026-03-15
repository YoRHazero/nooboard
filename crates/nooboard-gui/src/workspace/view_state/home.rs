use std::collections::BTreeSet;

use nooboard_core::{
    ClipboardRecord, ClipboardRecordSource, EventId, NetworkStatus, WorkspaceSnapshot,
};
use time::{OffsetDateTime, UtcOffset};

#[derive(Clone)]
pub struct HomePageViewState {
    pub system_core: HomeSystemCoreViewState,
}

#[derive(Clone)]
pub struct HomeSystemCoreViewState {
    pub local_device_id: String,
    pub network_control: HomeNetworkControlViewState,
    pub clipboard_control: HomeClipboardControlViewState,
    pub radar: HomeRadarViewState,
    pub clipboard: HomeClipboardPanelViewState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HomeNetworkControlViewState {
    pub can_start: bool,
    pub can_stop: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HomeClipboardControlViewState {
    pub adopt_event_id: Option<EventId>,
}

#[derive(Clone)]
pub struct HomeRadarViewState {
    pub state: HomeRadarVisualState,
    pub peers: Vec<HomeRadarPeerViewState>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HomeRadarVisualState {
    Running,
    Starting,
    Stopped,
    Error,
}

#[derive(Clone)]
pub struct HomeRadarPeerViewState {
    pub noob_id: String,
    pub address_label: String,
    pub transport_label: String,
    pub transferring: bool,
}

#[derive(Clone)]
pub struct HomeClipboardPanelViewState {
    pub latest_record: Option<HomeClipboardRecordViewState>,
}

#[derive(Clone)]
pub struct HomeClipboardRecordViewState {
    pub event_id: EventId,
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

pub(super) fn build_home_page_view_state(
    snapshot: &WorkspaceSnapshot,
    latest_record: Option<&ClipboardRecord>,
) -> HomePageViewState {
    let transferring_sessions = snapshot
        .network
        .transfers
        .active
        .iter()
        .map(|transfer| transfer.session_id)
        .collect::<BTreeSet<_>>();
    let latest_record = latest_record.map(|record| HomeClipboardRecordViewState {
        event_id: record.event_id,
        source: record.source,
        device_label: record.origin_device_id.clone(),
        recorded_at_label: clock_label_from_millis(record.created_at_ms),
        content: record.content.clone(),
    });

    HomePageViewState {
        system_core: HomeSystemCoreViewState {
            local_device_id: snapshot.identity.device_id.clone(),
            network_control: HomeNetworkControlViewState {
                can_start: matches!(snapshot.network.status, NetworkStatus::Stopped),
                can_stop: matches!(
                    snapshot.network.status,
                    NetworkStatus::Starting | NetworkStatus::Running | NetworkStatus::Error(_)
                ),
            },
            clipboard_control: HomeClipboardControlViewState {
                adopt_event_id: latest_record.as_ref().map(|record| record.event_id),
            },
            radar: HomeRadarViewState {
                state: radar_state(&snapshot.network.status),
                peers: snapshot
                    .network
                    .sessions
                    .iter()
                    .map(|session| HomeRadarPeerViewState {
                        noob_id: session.peer_noob_id.clone(),
                        address_label: session.remote_addr.to_string(),
                        transport_label: format!("{:?}", session.mode),
                        transferring: transferring_sessions.contains(&session.id),
                    })
                    .collect(),
            },
            clipboard: HomeClipboardPanelViewState { latest_record },
        },
    }
}

fn radar_state(status: &NetworkStatus) -> HomeRadarVisualState {
    match status {
        NetworkStatus::Running => HomeRadarVisualState::Running,
        NetworkStatus::Starting => HomeRadarVisualState::Starting,
        NetworkStatus::Stopped => HomeRadarVisualState::Stopped,
        NetworkStatus::Error(_) => HomeRadarVisualState::Error,
    }
}

fn clock_label_from_millis(timestamp_ms: i64) -> String {
    let offset = UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC);
    let nanos = i128::from(timestamp_ms) * 1_000_000;
    let datetime = OffsetDateTime::from_unix_timestamp_nanos(nanos)
        .unwrap_or(OffsetDateTime::UNIX_EPOCH)
        .to_offset(offset);

    format!(
        "{:02}:{:02}:{:02}",
        datetime.hour(),
        datetime.minute(),
        datetime.second()
    )
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
    fn build_home_page_projects_system_core_state() {
        let snapshot = sample_snapshot(NetworkStatus::Running);
        let latest_record = ClipboardRecord {
            event_id: EventId::new(),
            source: ClipboardRecordSource::RemoteSync,
            origin_noob_id: NoobId::new("remote-node"),
            origin_device_id: "remote-mbp".to_string(),
            created_at_ms: 0,
            applied_at_ms: 0,
            content: "remote text".to_string(),
        };

        let state = build_home_page_view_state(&snapshot, Some(&latest_record));

        assert_eq!(state.system_core.local_device_id, "desk-01");
        assert!(state.system_core.network_control.can_stop);
        assert_eq!(
            state.system_core.clipboard_control.adopt_event_id,
            Some(latest_record.event_id)
        );
        assert!(matches!(
            state
                .system_core
                .clipboard
                .latest_record
                .as_ref()
                .map(|record| record.source),
            Some(ClipboardRecordSource::RemoteSync)
        ));
    }

    #[test]
    fn build_home_page_handles_empty_clipboard_and_stopped_network() {
        let snapshot = sample_snapshot(NetworkStatus::Stopped);

        let state = build_home_page_view_state(&snapshot, None);

        assert!(state.system_core.network_control.can_start);
        assert_eq!(state.system_core.clipboard_control.adopt_event_id, None);
        assert!(matches!(
            state.system_core.radar.state,
            HomeRadarVisualState::Stopped
        ));
        assert!(state.system_core.clipboard.latest_record.is_none());
    }

    fn sample_snapshot(status: NetworkStatus) -> WorkspaceSnapshot {
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
                status,
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
