use std::collections::{BTreeMap, BTreeSet};
use std::time::{SystemTime, UNIX_EPOCH};

use nooboard_app::{
    AppState, CompletedTransfer, Transfer, TransferDirection, TransferOutcome, TransferState,
};

use crate::state::live_app::LiveAppStore;
use crate::ui::workspace::view::shared::clock_label_from_millis;

#[derive(Clone)]
pub(in crate::ui::workspace::view) struct TransfersSnapshot {
    pub metrics: TransferMetricsSnapshot,
    pub download_dir_label: String,
    pub target_peers: Vec<TransferTargetSnapshot>,
    pub incoming_pending: Vec<IncomingTransferCardSnapshot>,
    pub active_uploads: Vec<ActiveTransferCardSnapshot>,
    pub active_downloads: Vec<ActiveTransferCardSnapshot>,
    pub completed_uploads: Vec<CompletedTransferCardSnapshot>,
    pub completed_downloads: Vec<CompletedTransferCardSnapshot>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::ui::workspace::view) struct TransferMetricsSnapshot {
    pub awaiting: usize,
    pub active: usize,
    pub completed: usize,
}

#[derive(Clone)]
pub(in crate::ui::workspace::view) struct TransferTargetSnapshot {
    pub noob_id: String,
    pub device_id: String,
    pub selected: bool,
}

#[derive(Clone)]
pub(in crate::ui::workspace::view) struct IncomingTransferCardSnapshot {
    pub transfer_id: nooboard_app::TransferId,
    pub peer_device_id: String,
    pub file_name: String,
    pub file_size_label: String,
    pub offered_at_label: String,
}

#[derive(Clone)]
pub(in crate::ui::workspace::view) struct ActiveTransferCardSnapshot {
    pub transfer_id: nooboard_app::TransferId,
    pub peer_device_id: String,
    pub file_name: String,
    pub file_size_label: String,
    pub transferred_label: String,
    pub progress_fraction: f32,
    pub progress_percent_label: String,
    pub state_label: &'static str,
    pub state_accent: TransferVisualAccent,
    pub started_at_label: String,
    pub updated_at_label: String,
    pub speed_label: Option<String>,
    pub eta_label: Option<String>,
}

#[derive(Clone)]
pub(in crate::ui::workspace::view) struct CompletedTransferCardSnapshot {
    pub peer_device_id: String,
    pub file_name: String,
    pub file_size_label: String,
    pub outcome_label: &'static str,
    pub outcome_accent: TransferVisualAccent,
    pub finished_at_label: String,
    pub duration_label: Option<String>,
    pub saved_path_label: Option<String>,
    pub message: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::ui::workspace::view) enum TransferVisualAccent {
    Amber,
    Blue,
    Green,
    Rose,
}

pub(in crate::ui::workspace::view) fn build_transfers_snapshot(
    store: &LiveAppStore,
    selected_target_noob_ids: &BTreeSet<String>,
) -> TransfersSnapshot {
    let app_state = store.app_state();
    let peer_labels = peer_labels(app_state);
    let now_ms = now_millis();

    let target_peers = app_state
        .peers
        .connected
        .iter()
        .map(|peer| TransferTargetSnapshot {
            noob_id: peer.noob_id.as_str().to_string(),
            device_id: peer.device_id.clone(),
            selected: selected_target_noob_ids.contains(peer.noob_id.as_str()),
        })
        .collect();

    let incoming_pending = app_state
        .transfers
        .incoming_pending
        .iter()
        .map(|transfer| IncomingTransferCardSnapshot {
            transfer_id: transfer.transfer_id.clone(),
            peer_device_id: transfer.peer_device_id.clone(),
            file_name: transfer.file_name.clone(),
            file_size_label: bytes_to_label(transfer.file_size),
            offered_at_label: clock_label_from_millis(transfer.offered_at_ms),
        })
        .collect::<Vec<_>>();

    let active_cards = app_state
        .transfers
        .active
        .iter()
        .map(|transfer| active_transfer_snapshot(store, &peer_labels, transfer, now_ms))
        .collect::<Vec<_>>();
    let (active_uploads, active_downloads) = split_by_direction(active_cards);

    let completed_cards = app_state
        .transfers
        .recent_completed
        .iter()
        .map(|transfer| completed_transfer_snapshot(&peer_labels, transfer))
        .collect::<Vec<_>>();
    let (completed_uploads, completed_downloads) = split_completed_by_direction(completed_cards);

    TransfersSnapshot {
        metrics: TransferMetricsSnapshot {
            awaiting: app_state.transfers.incoming_pending.len(),
            active: app_state.transfers.active.len(),
            completed: app_state.transfers.recent_completed.len(),
        },
        download_dir_label: app_state
            .settings
            .transfers
            .download_dir
            .display()
            .to_string(),
        target_peers,
        incoming_pending,
        active_uploads,
        active_downloads,
        completed_uploads,
        completed_downloads,
    }
}

fn peer_labels(app_state: &AppState) -> BTreeMap<&str, &str> {
    app_state
        .peers
        .connected
        .iter()
        .map(|peer| (peer.noob_id.as_str(), peer.device_id.as_str()))
        .collect()
}

fn active_transfer_snapshot(
    store: &LiveAppStore,
    peer_labels: &BTreeMap<&str, &str>,
    transfer: &Transfer,
    now_ms: i64,
) -> (TransferDirection, ActiveTransferCardSnapshot) {
    let progress_fraction = if transfer.file_size == 0 {
        0.0
    } else {
        (transfer.transferred_bytes as f32 / transfer.file_size as f32).clamp(0.0, 1.0)
    };
    let estimate = store.transfer_telemetry().estimate_for(transfer, now_ms);

    (
        transfer.direction,
        ActiveTransferCardSnapshot {
            transfer_id: transfer.transfer_id.clone(),
            peer_device_id: peer_display_label(
                peer_labels,
                transfer.peer_noob_id.as_str(),
                &transfer.peer_device_id,
            ),
            file_name: fallback_file_name(&transfer.file_name),
            file_size_label: bytes_to_label(transfer.file_size),
            transferred_label: format!(
                "{} / {}",
                bytes_to_label(transfer.transferred_bytes),
                bytes_to_label(transfer.file_size)
            ),
            progress_fraction,
            progress_percent_label: format!("{:.0}%", progress_fraction * 100.0),
            state_label: transfer_state_label(transfer.state),
            state_accent: transfer_state_accent(transfer.state),
            started_at_label: clock_label_from_millis(transfer.started_at_ms),
            updated_at_label: clock_label_from_millis(transfer.updated_at_ms),
            speed_label: estimate.map(|estimate| speed_label(estimate.speed_bps)),
            eta_label: estimate.and_then(|estimate| estimate.eta_seconds.map(eta_label)),
        },
    )
}

fn completed_transfer_snapshot(
    peer_labels: &BTreeMap<&str, &str>,
    transfer: &CompletedTransfer,
) -> (TransferDirection, CompletedTransferCardSnapshot) {
    (
        transfer.direction,
        CompletedTransferCardSnapshot {
            peer_device_id: peer_display_label(
                peer_labels,
                transfer.peer_noob_id.as_str(),
                &transfer.peer_device_id,
            ),
            file_name: fallback_file_name(&transfer.file_name),
            file_size_label: bytes_to_label(transfer.file_size),
            outcome_label: transfer_outcome_label(transfer.outcome),
            outcome_accent: transfer_outcome_accent(transfer.outcome),
            finished_at_label: clock_label_from_millis(transfer.finished_at_ms),
            duration_label: transfer
                .started_at_ms
                .map(|started_at_ms| duration_label(transfer.finished_at_ms - started_at_ms)),
            saved_path_label: transfer
                .saved_path
                .as_ref()
                .map(|path| path.display().to_string()),
            message: transfer.message.clone(),
        },
    )
}

fn split_by_direction<T>(items: Vec<(TransferDirection, T)>) -> (Vec<T>, Vec<T>) {
    let mut uploads = Vec::new();
    let mut downloads = Vec::new();
    for (direction, item) in items {
        match direction {
            TransferDirection::Upload => uploads.push(item),
            TransferDirection::Download => downloads.push(item),
        }
    }
    (uploads, downloads)
}

fn split_completed_by_direction<T>(items: Vec<(TransferDirection, T)>) -> (Vec<T>, Vec<T>) {
    split_by_direction(items)
}

fn peer_display_label(
    peer_labels: &BTreeMap<&str, &str>,
    peer_noob_id: &str,
    peer_device_id: &str,
) -> String {
    peer_labels
        .get(peer_noob_id)
        .copied()
        .unwrap_or(peer_device_id)
        .to_string()
}

fn fallback_file_name(file_name: &str) -> String {
    if file_name.is_empty() {
        "pending metadata".to_string()
    } else {
        file_name.to_string()
    }
}

fn transfer_state_label(state: TransferState) -> &'static str {
    match state {
        TransferState::Queued => "Queued",
        TransferState::Starting => "Starting",
        TransferState::InProgress => "In Progress",
        TransferState::Cancelling => "Cancelling",
    }
}

fn transfer_state_accent(state: TransferState) -> TransferVisualAccent {
    match state {
        TransferState::Queued => TransferVisualAccent::Amber,
        TransferState::Starting => TransferVisualAccent::Blue,
        TransferState::InProgress => TransferVisualAccent::Blue,
        TransferState::Cancelling => TransferVisualAccent::Rose,
    }
}

fn transfer_outcome_label(outcome: TransferOutcome) -> &'static str {
    match outcome {
        TransferOutcome::Succeeded => "Succeeded",
        TransferOutcome::Rejected => "Rejected",
        TransferOutcome::Cancelled => "Cancelled",
        TransferOutcome::Failed => "Failed",
    }
}

fn transfer_outcome_accent(outcome: TransferOutcome) -> TransferVisualAccent {
    match outcome {
        TransferOutcome::Succeeded => TransferVisualAccent::Green,
        TransferOutcome::Rejected => TransferVisualAccent::Amber,
        TransferOutcome::Cancelled => TransferVisualAccent::Rose,
        TransferOutcome::Failed => TransferVisualAccent::Rose,
    }
}

fn bytes_to_label(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;

    let bytes_f = bytes as f64;
    if bytes_f >= GB {
        format!("{:.1} GB", bytes_f / GB)
    } else if bytes_f >= MB {
        format!("{:.1} MB", bytes_f / MB)
    } else if bytes_f >= KB {
        format!("{:.1} KB", bytes_f / KB)
    } else {
        format!("{} B", bytes)
    }
}

fn speed_label(bytes_per_second: u64) -> String {
    format!("{}/s", bytes_to_label(bytes_per_second))
}

fn eta_label(seconds: u64) -> String {
    format!("ETA {}", duration_label(seconds as i64 * 1_000))
}

fn duration_label(duration_ms: i64) -> String {
    let total_seconds = duration_ms.max(0).div_euclid(1_000);
    let minutes = total_seconds / 60;
    let seconds = total_seconds % 60;

    if minutes > 0 {
        format!("{minutes}m {seconds:02}s")
    } else {
        format!("{seconds}s")
    }
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| i64::try_from(duration.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use nooboard_app::{
        AppState, ClipboardSettings, ClipboardState, CompletedTransfer, ConnectedPeer,
        ConnectionIdentitySettings, IncomingTransfer, LocalConnectionInfo, LocalIdentity,
        NetworkSettings, NoobId, PeerTransport, PeersState, SettingsState, StorageSettings,
        SyncActualStatus, SyncDesiredState, SyncState, Transfer, TransferDirection,
        TransferOutcome, TransferSettings, TransferState, TransfersState,
    };

    use crate::state::live_app::LiveAppStore;

    use super::*;

    fn sample_store() -> LiveAppStore {
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
                desired: SyncDesiredState::Running,
                actual: SyncActualStatus::Running,
            },
            peers: PeersState {
                connected: vec![
                    ConnectedPeer {
                        noob_id: NoobId::new("peer-a"),
                        device_id: "mbp-lab".to_string(),
                        addresses: vec!["127.0.0.1:17001".parse().unwrap()],
                        transport: PeerTransport::Manual,
                        latency_ms: Some(18),
                    },
                    ConnectedPeer {
                        noob_id: NoobId::new("peer-b"),
                        device_id: "render-node".to_string(),
                        addresses: vec!["127.0.0.1:17002".parse().unwrap()],
                        transport: PeerTransport::Mdns,
                        latency_ms: None,
                    },
                ],
            },
            clipboard: ClipboardState::default(),
            transfers: TransfersState {
                incoming_pending: vec![IncomingTransfer {
                    transfer_id: nooboard_app::TransferId::new(NoobId::new("peer-a"), 7),
                    peer_noob_id: NoobId::new("peer-a"),
                    peer_device_id: "mbp-lab".to_string(),
                    file_name: "photo.jpg".to_string(),
                    file_size: 2_048,
                    total_chunks: 3,
                    offered_at_ms: 1_000,
                }],
                active: vec![
                    Transfer {
                        transfer_id: nooboard_app::TransferId::new(NoobId::new("peer-a"), 1),
                        direction: TransferDirection::Upload,
                        peer_noob_id: NoobId::new("peer-a"),
                        peer_device_id: "mbp-lab".to_string(),
                        file_name: "report.pdf".to_string(),
                        file_size: 10_000,
                        transferred_bytes: 4_000,
                        state: TransferState::InProgress,
                        started_at_ms: 0,
                        updated_at_ms: 500,
                    },
                    Transfer {
                        transfer_id: nooboard_app::TransferId::new(NoobId::new("peer-b"), 2),
                        direction: TransferDirection::Download,
                        peer_noob_id: NoobId::new("peer-b"),
                        peer_device_id: "render-node".to_string(),
                        file_name: "archive.zip".to_string(),
                        file_size: 20_000,
                        transferred_bytes: 1_000,
                        state: TransferState::Starting,
                        started_at_ms: 200,
                        updated_at_ms: 400,
                    },
                ],
                recent_completed: vec![CompletedTransfer {
                    transfer_id: nooboard_app::TransferId::new(NoobId::new("peer-a"), 3),
                    direction: TransferDirection::Download,
                    peer_noob_id: NoobId::new("peer-a"),
                    peer_device_id: "mbp-lab".to_string(),
                    file_name: "done.txt".to_string(),
                    file_size: 4_096,
                    outcome: TransferOutcome::Succeeded,
                    started_at_ms: Some(0),
                    finished_at_ms: 5_000,
                    saved_path: Some(PathBuf::from("/tmp/done.txt")),
                    message: None,
                }],
            },
            settings: SettingsState {
                connection_identity: ConnectionIdentitySettings {
                    device_id: "desk-01".to_string(),
                    token: "dev-sync-token".to_string(),
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

    #[test]
    fn snapshot_splits_transfers_by_direction() {
        let store = sample_store();
        let snapshot = build_transfers_snapshot(&store, &BTreeSet::new());

        assert_eq!(snapshot.metrics.awaiting, 1);
        assert_eq!(snapshot.metrics.active, 2);
        assert_eq!(snapshot.metrics.completed, 1);
        assert_eq!(snapshot.active_uploads.len(), 1);
        assert_eq!(snapshot.active_downloads.len(), 1);
        assert_eq!(snapshot.completed_downloads.len(), 1);
    }

    #[test]
    fn snapshot_marks_selected_targets() {
        let store = sample_store();
        let selected = BTreeSet::from(["peer-b".to_string()]);
        let snapshot = build_transfers_snapshot(&store, &selected);

        assert!(
            snapshot
                .target_peers
                .iter()
                .find(|target| target.noob_id == "peer-b")
                .expect("peer-b target")
                .selected
        );
    }
}
