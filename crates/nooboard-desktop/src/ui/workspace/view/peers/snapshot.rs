use std::collections::{BTreeMap, BTreeSet};

use nooboard_app::{AppState, ConnectedPeer, PeerTransport};

use crate::state::live_app::LiveAppStore;

use super::page_state::PeersFilter;

#[derive(Clone)]
pub(super) struct PeersSnapshot {
    pub filter: PeersFilter,
    pub counts: PeerFilterCounts,
    pub duplicate_warning: Option<PeerDuplicateWarning>,
    pub rows: Vec<PeerRowSnapshot>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct PeerFilterCounts {
    pub all: usize,
    pub idle: usize,
    pub transferring: usize,
}

#[derive(Clone)]
pub(super) struct PeerDuplicateWarning {
    pub duplicate_labels: Vec<String>,
    pub affected_peer_count: usize,
    pub affects_local_identity: bool,
}

#[derive(Clone)]
pub(super) struct PeerRowSnapshot {
    pub device_id: String,
    pub noob_id: String,
    pub endpoint_label: String,
    pub endpoint_detail: String,
    pub status: PeerVisualStatus,
    pub duplicate_device_id: bool,
    pub duplicates_local_identity: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PeerVisualStatus {
    Connected,
    Transferring,
}

impl PeerDuplicateWarning {
    pub fn title(&self) -> &'static str {
        "Duplicate device labels detected"
    }

    pub fn detail(&self) -> String {
        let labels = self.duplicate_labels.join(", ");
        if self.affects_local_identity {
            format!(
                "{} connected peers share labels with each other or with this device: {}",
                self.affected_peer_count, labels
            )
        } else {
            format!(
                "{} connected peers share labels: {}",
                self.affected_peer_count, labels
            )
        }
    }
}

pub(super) fn build_peers_snapshot(store: &LiveAppStore, filter: PeersFilter) -> PeersSnapshot {
    let app_state = store.app_state();
    let transferring_peer_ids: BTreeSet<_> = app_state
        .transfers
        .active
        .iter()
        .map(|transfer| transfer.peer_noob_id.clone())
        .collect();

    let duplicate_device_ids = duplicate_device_ids(app_state);
    let local_device_id = app_state.identity.device_id.as_str();

    let mut idle = 0usize;
    let mut transferring = 0usize;
    let mut rows = Vec::with_capacity(app_state.peers.connected.len());

    for peer in &app_state.peers.connected {
        let status = if transferring_peer_ids.contains(&peer.noob_id) {
            transferring += 1;
            PeerVisualStatus::Transferring
        } else {
            idle += 1;
            PeerVisualStatus::Connected
        };

        if !filter.matches(status) {
            continue;
        }

        let duplicate_device_id = duplicate_device_ids.contains(peer.device_id.as_str());
        rows.push(PeerRowSnapshot {
            device_id: peer.device_id.clone(),
            noob_id: peer.noob_id.as_str().to_string(),
            endpoint_label: peer
                .addresses
                .first()
                .map(ToString::to_string)
                .unwrap_or_else(|| "unknown address".to_string()),
            endpoint_detail: endpoint_detail(peer),
            status,
            duplicate_device_id,
            duplicates_local_identity: duplicate_device_id && peer.device_id == local_device_id,
        });
    }

    let duplicate_warning = if duplicate_device_ids.is_empty() {
        None
    } else {
        let affected_peer_count = app_state
            .peers
            .connected
            .iter()
            .filter(|peer| duplicate_device_ids.contains(peer.device_id.as_str()))
            .count();
        Some(PeerDuplicateWarning {
            duplicate_labels: duplicate_device_ids.iter().cloned().collect(),
            affected_peer_count,
            affects_local_identity: duplicate_device_ids.contains(local_device_id),
        })
    };

    PeersSnapshot {
        filter,
        counts: PeerFilterCounts {
            all: app_state.peers.connected.len(),
            idle,
            transferring,
        },
        duplicate_warning,
        rows,
    }
}

fn endpoint_detail(peer: &ConnectedPeer) -> String {
    let mut detail = transport_label(peer.transport).to_string();
    if let Some(latency_ms) = peer.latency_ms {
        detail.push_str(&format!(" · {latency_ms} ms"));
    }
    if peer.addresses.len() > 1 {
        detail.push_str(&format!(" · +{} more", peer.addresses.len() - 1));
    }
    detail
}

fn duplicate_device_ids(app_state: &AppState) -> BTreeSet<String> {
    let mut counts = BTreeMap::<String, usize>::new();
    *counts
        .entry(app_state.identity.device_id.clone())
        .or_default() += 1;
    for peer in &app_state.peers.connected {
        *counts.entry(peer.device_id.clone()).or_default() += 1;
    }

    counts
        .into_iter()
        .filter_map(|(device_id, count)| (count > 1).then_some(device_id))
        .collect()
}

fn transport_label(transport: PeerTransport) -> &'static str {
    match transport {
        PeerTransport::Mdns => "mDNS",
        PeerTransport::Manual => "Manual",
        PeerTransport::Mixed => "Mixed",
        PeerTransport::Unknown => "Unknown",
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use nooboard_app::{
        AppState, ClipboardSettings, ClipboardState, ConnectedPeer, LocalIdentity, NetworkSettings,
        NoobId, PeerTransport, PeersState, SettingsState, StorageSettings, SyncActualStatus,
        SyncDesiredState, SyncState, Transfer, TransferDirection, TransferId, TransferSettings,
        TransferState, TransfersState,
    };

    use super::*;

    fn sample_store(local_device_id: &str) -> LiveAppStore {
        let app_state = AppState {
            revision: 1,
            identity: LocalIdentity {
                noob_id: NoobId::new("local-node"),
                device_id: local_device_id.to_string(),
            },
            sync: SyncState {
                desired: SyncDesiredState::Running,
                actual: SyncActualStatus::Running,
            },
            peers: PeersState {
                connected: vec![
                    ConnectedPeer {
                        noob_id: NoobId::new("alpha"),
                        device_id: "mbp-lab".to_string(),
                        addresses: vec!["127.0.0.1:17001".parse().unwrap()],
                        transport: PeerTransport::Manual,
                        latency_ms: Some(18),
                    },
                    ConnectedPeer {
                        noob_id: NoobId::new("beta"),
                        device_id: "render-node".to_string(),
                        addresses: vec!["127.0.0.1:17002".parse().unwrap()],
                        transport: PeerTransport::Mdns,
                        latency_ms: None,
                    },
                    ConnectedPeer {
                        noob_id: NoobId::new("gamma"),
                        device_id: "mbp-lab".to_string(),
                        addresses: vec!["127.0.0.1:17003".parse().unwrap()],
                        transport: PeerTransport::Mixed,
                        latency_ms: Some(41),
                    },
                ],
            },
            clipboard: ClipboardState::default(),
            transfers: TransfersState {
                incoming_pending: Vec::new(),
                active: vec![Transfer {
                    transfer_id: TransferId::new(NoobId::new("beta"), 1),
                    direction: TransferDirection::Upload,
                    peer_noob_id: NoobId::new("beta"),
                    peer_device_id: "render-node".to_string(),
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
                network: NetworkSettings {
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
    fn transfer_activity_derives_transferring_rows_and_counts() {
        let store = sample_store("desk-01");
        let snapshot = build_peers_snapshot(&store, PeersFilter::All);

        assert_eq!(
            snapshot.counts,
            PeerFilterCounts {
                all: 3,
                idle: 2,
                transferring: 1,
            }
        );
        assert_eq!(
            snapshot
                .rows
                .iter()
                .find(|row| row.noob_id == "beta")
                .map(|row| row.status),
            Some(PeerVisualStatus::Transferring)
        );
    }

    #[test]
    fn duplicate_peer_device_ids_are_marked() {
        let store = sample_store("desk-01");
        let snapshot = build_peers_snapshot(&store, PeersFilter::All);

        let warning = snapshot.duplicate_warning.expect("duplicate warning");
        assert_eq!(warning.duplicate_labels, vec!["mbp-lab".to_string()]);
        assert_eq!(warning.affected_peer_count, 2);
        assert!(!warning.affects_local_identity);
        assert_eq!(
            snapshot
                .rows
                .iter()
                .filter(|row| row.duplicate_device_id)
                .count(),
            2
        );
    }

    #[test]
    fn duplicate_with_local_identity_is_marked() {
        let store = sample_store("render-node");
        let snapshot = build_peers_snapshot(&store, PeersFilter::All);

        let warning = snapshot.duplicate_warning.expect("duplicate warning");
        assert!(warning.affects_local_identity);
        assert!(
            snapshot
                .rows
                .iter()
                .find(|row| row.noob_id == "beta")
                .expect("beta row")
                .duplicates_local_identity
        );
    }

    #[test]
    fn idle_filter_excludes_transferring_rows() {
        let store = sample_store("desk-01");
        let snapshot = build_peers_snapshot(&store, PeersFilter::Idle);

        assert_eq!(snapshot.rows.len(), 2);
        assert!(
            snapshot
                .rows
                .iter()
                .all(|row| row.status == PeerVisualStatus::Connected)
        );
    }
}
