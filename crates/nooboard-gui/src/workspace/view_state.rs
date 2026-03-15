use nooboard_core::{ClipboardRecord, ClipboardRecordSource, NetworkStatus, WorkspaceSnapshot};

#[derive(Clone)]
pub struct WorkspaceViewState {
    pub headline: String,
    pub subheadline: String,
    pub revision_label: String,
    pub network_status: String,
    pub network_can_start: bool,
    pub network_can_stop: bool,
    pub metrics: Vec<WorkspaceMetricViewState>,
    pub latest_clipboard_preview: String,
    pub latest_clipboard_event_id: Option<String>,
    pub latest_clipboard_source: Option<String>,
    pub can_adopt_latest: bool,
    pub lan_peers: Vec<String>,
    pub direct_seeds: Vec<String>,
    pub pending_requests: Vec<String>,
    pub sessions: Vec<String>,
    pub incoming_transfers: Vec<String>,
    pub active_transfers: Vec<String>,
    pub completed_transfers: Vec<String>,
    pub settings_rows: Vec<WorkspaceMetricViewState>,
}

#[derive(Clone)]
pub struct WorkspaceMetricViewState {
    pub label: &'static str,
    pub value: String,
}

pub fn build_workspace_view_state(
    snapshot: &WorkspaceSnapshot,
    latest_record: Option<&ClipboardRecord>,
) -> WorkspaceViewState {
    let network_status = network_status_label(&snapshot.network.status);

    WorkspaceViewState {
        headline: snapshot.identity.device_id.clone(),
        subheadline: snapshot.identity.noob_id.to_string(),
        revision_label: format!("revision {}", snapshot.revision),
        network_can_start: matches!(snapshot.network.status, NetworkStatus::Stopped),
        network_can_stop: matches!(
            snapshot.network.status,
            NetworkStatus::Starting | NetworkStatus::Running | NetworkStatus::Error(_)
        ),
        network_status: network_status.clone(),
        metrics: vec![
            WorkspaceMetricViewState {
                label: "Network",
                value: network_status,
            },
            WorkspaceMetricViewState {
                label: "LAN Peers",
                value: snapshot.network.lan_peers.len().to_string(),
            },
            WorkspaceMetricViewState {
                label: "Direct Seeds",
                value: snapshot.network.direct_seeds.len().to_string(),
            },
            WorkspaceMetricViewState {
                label: "Requests",
                value: snapshot.network.pending_direct_requests.len().to_string(),
            },
            WorkspaceMetricViewState {
                label: "Sessions",
                value: snapshot.network.sessions.len().to_string(),
            },
            WorkspaceMetricViewState {
                label: "Transfers",
                value: format!(
                    "{} active / {} pending / {} complete",
                    snapshot.network.transfers.active.len(),
                    snapshot.network.transfers.incoming_pending.len(),
                    snapshot.network.transfers.recent_completed.len()
                ),
            },
            WorkspaceMetricViewState {
                label: "Endpoint",
                value: snapshot
                    .local_connection
                    .device_endpoint
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "Unavailable".to_string()),
            },
        ],
        latest_clipboard_preview: latest_record
            .map(|record| clipboard_preview(&record.content))
            .unwrap_or_else(|| "No committed clipboard record yet.".to_string()),
        latest_clipboard_event_id: latest_record.map(|record| record.event_id.to_string()),
        latest_clipboard_source: latest_record
            .map(|record| clipboard_source_label(record.source).to_string()),
        can_adopt_latest: latest_record.is_some(),
        lan_peers: snapshot
            .network
            .lan_peers
            .iter()
            .map(|peer| {
                let address = peer
                    .addresses
                    .first()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "unknown".to_string());
                format!("{} · {} · {}", peer.device_id, address, peer.noob_id)
            })
            .collect(),
        direct_seeds: snapshot
            .network
            .direct_seeds
            .iter()
            .map(|seed| {
                format!(
                    "{} · {}:{} · {}",
                    seed.label,
                    seed.host,
                    seed.port,
                    if seed.enabled { "enabled" } else { "disabled" }
                )
            })
            .collect(),
        pending_requests: snapshot
            .network
            .pending_direct_requests
            .iter()
            .map(|request| {
                format!(
                    "{} · {} · {}",
                    request.peer_device_id, request.remote_addr, request.peer_noob_id
                )
            })
            .collect(),
        sessions: snapshot
            .network
            .sessions
            .iter()
            .map(|session| {
                format!(
                    "{} · {:?} · {}",
                    session.peer_device_id, session.mode, session.remote_addr
                )
            })
            .collect(),
        incoming_transfers: snapshot
            .network
            .transfers
            .incoming_pending
            .iter()
            .map(|transfer| {
                format!(
                    "{} · {} bytes · {}",
                    transfer.file_name, transfer.file_size, transfer.peer_device_id
                )
            })
            .collect(),
        active_transfers: snapshot
            .network
            .transfers
            .active
            .iter()
            .map(|transfer| {
                format!(
                    "{} · {} / {} bytes · {:?}",
                    transfer.file_name,
                    transfer.transferred_bytes,
                    transfer.file_size,
                    transfer.state
                )
            })
            .collect(),
        completed_transfers: snapshot
            .network
            .transfers
            .recent_completed
            .iter()
            .map(|transfer| {
                format!(
                    "{} · {:?} · {}",
                    transfer.file_name, transfer.outcome, transfer.peer_device_id
                )
            })
            .collect(),
        settings_rows: vec![
            WorkspaceMetricViewState {
                label: "Device ID",
                value: snapshot.settings.connection.device_id.clone(),
            },
            WorkspaceMetricViewState {
                label: "Token",
                value: snapshot.settings.connection.token.clone(),
            },
            WorkspaceMetricViewState {
                label: "Listen Port",
                value: snapshot.settings.network.listen_port.to_string(),
            },
            WorkspaceMetricViewState {
                label: "LAN Enabled",
                value: snapshot.settings.network.lan_enabled.to_string(),
            },
            WorkspaceMetricViewState {
                label: "Local Capture",
                value: snapshot.settings.clipboard.local_capture_enabled.to_string(),
            },
            WorkspaceMetricViewState {
                label: "Download Dir",
                value: snapshot.settings.transfers.download_dir.display().to_string(),
            },
            WorkspaceMetricViewState {
                label: "History Window",
                value: snapshot.settings.storage.history_window_days.to_string(),
            },
        ],
    }
}

fn network_status_label(status: &NetworkStatus) -> String {
    match status {
        NetworkStatus::Stopped => "Stopped".to_string(),
        NetworkStatus::Starting => "Starting".to_string(),
        NetworkStatus::Running => "Running".to_string(),
        NetworkStatus::Error(message) => format!("Error: {message}"),
    }
}

fn clipboard_preview(content: &str) -> String {
    let preview = content.replace('\n', " ");
    let mut chars = preview.chars();
    let compact = chars.by_ref().take(140).collect::<String>();
    if chars.next().is_some() {
        format!("{compact}…")
    } else {
        compact
    }
}

fn clipboard_source_label(source: ClipboardRecordSource) -> &'static str {
    match source {
        ClipboardRecordSource::LocalCapture => "Local Capture",
        ClipboardRecordSource::RemoteSync => "Remote Sync",
        ClipboardRecordSource::UserSubmit => "User Submit",
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

        assert_eq!(state.headline, "desk-01");
        assert!(state.network_can_stop);
        assert_eq!(state.latest_clipboard_source.as_deref(), Some("User Submit"));
        assert_eq!(state.settings_rows[0].label, "Device ID");
    }

    #[test]
    fn build_workspace_view_state_handles_missing_clipboard_record() {
        let snapshot = sample_snapshot();

        let state = build_workspace_view_state(&snapshot, None);

        assert_eq!(state.latest_clipboard_preview, "No committed clipboard record yet.");
        assert!(!state.can_adopt_latest);
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
