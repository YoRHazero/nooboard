use nooboard_app::{AppSyncStatus, SyncDesiredState};

#[derive(Clone)]
pub struct SharedState {
    pub app: AppStore,
}

impl SharedState {
    pub fn demo() -> Self {
        Self {
            app: AppStore {
                desired_state: SyncDesiredState::Running,
                sync_status: AppSyncStatus::Running,
                online_peers: 3,
                manual_peers: 2,
                system_core: SystemCoreStore {
                    local_device_id: "desk-01".into(),
                    network_enabled: true,
                    auto_bridge_remote_text: false,
                    peers: vec![
                        SystemPeer {
                            node_id: "b71d8bb8-e35b-4e2e-9278-6d99ab77b6cf".into(),
                            device_id: "mbp-lab".into(),
                            ip: "192.168.31.18:17890".into(),
                            status: SystemPeerStatus::Connected,
                        },
                        SystemPeer {
                            node_id: "3177c7f4-d664-4a5a-823e-8e5a962f34f1".into(),
                            device_id: "render-node".into(),
                            ip: "192.168.31.44:17890".into(),
                            status: SystemPeerStatus::Transferring,
                        },
                        SystemPeer {
                            node_id: "8d9c9bc0-b070-4b7e-89f9-4712929b89b5".into(),
                            device_id: "mbp-lab".into(),
                            ip: "192.168.31.52:17890".into(),
                            status: SystemPeerStatus::Connected,
                        },
                    ],
                    local_clipboard: ClipboardSnapshot {
                        origin: ClipboardOrigin::Local,
                        device_id: "desk-01".into(),
                        updated_at_order: 2026_03_03_12_14_08,
                        captured_at_label: "2026-03-03 12:14:08".into(),
                        content: "cargo test -p nooboard-desktop --workspace".into(),
                    },
                    latest_remote_clipboard: Some(ClipboardSnapshot {
                        origin: ClipboardOrigin::Remote,
                        device_id: "render-node".into(),
                        updated_at_order: 2026_03_03_12_16_27,
                        captured_at_label: "2026-03-03 12:16:27".into(),
                        content: "handoff ready: transfer lane 02 is draining and db write is still pending".into(),
                    }),
                },
                pending_files: vec![
                    PendingFileDecision {
                        file_name: "report-quarterly-final-v7-with-annotations.pdf".into(),
                        peer_label: "node-alpha".into(),
                        size_label: "14.2 MB".into(),
                    },
                    PendingFileDecision {
                        file_name: "archive-project-handshake-debug-bundle.zip".into(),
                        peer_label: "node-beta".into(),
                        size_label: "88.5 MB".into(),
                    },
                ],
                recent_activity: vec![
                    ActivityItem {
                        time_label: "12:03".into(),
                        kind: "TextReceived".into(),
                        title: "device-a sent \"alpha release checklist updated after handoff\"".into(),
                        detail: "stored to history and promoted to the latest clipboard snapshot".into(),
                    },
                    ActivityItem {
                        time_label: "12:08".into(),
                        kind: "ConnectionError".into(),
                        title: "192.168.1.9 rejected handshake during capability negotiation".into(),
                        detail: "retry budget is active and the runtime is waiting for a fresh mdns advertisement".into(),
                    },
                    ActivityItem {
                        time_label: "12:10".into(),
                        kind: "TransferFinished".into(),
                        title: "report-quarterly-final-v7-with-annotations.pdf downloaded".into(),
                        detail: "saved to downloads and indexed for the next recent history preview".into(),
                    },
                ],
                transfers: vec![
                    TransferItem {
                        file_name: "file-a.zip".into(),
                        progress: 0.61,
                        bytes_label: "92 MB / 150 MB".into(),
                        speed_label: "3.2 MB/s".into(),
                        eta_label: "ETA 18s".into(),
                    },
                    TransferItem {
                        file_name: "image.png".into(),
                        progress: 0.24,
                        bytes_label: "6 MB / 25 MB".into(),
                        speed_label: "1.1 MB/s".into(),
                        eta_label: "ETA 14s".into(),
                    },
                ],
                recent_history: vec![
                    "alpha release checklist updated after handoff".into(),
                    "build finished".into(),
                    "ssh key copied".into(),
                    "remote message from node-gamma with a long troubleshooting summary".into(),
                    "deploy command with temporary feature flags and validation notes".into(),
                ],
            },
        }
    }
}

#[derive(Clone)]
pub struct AppStore {
    pub desired_state: SyncDesiredState,
    pub sync_status: AppSyncStatus,
    pub online_peers: usize,
    pub manual_peers: usize,
    pub system_core: SystemCoreStore,
    pub pending_files: Vec<PendingFileDecision>,
    pub recent_activity: Vec<ActivityItem>,
    pub transfers: Vec<TransferItem>,
    pub recent_history: Vec<String>,
}

#[derive(Clone)]
pub struct SystemCoreStore {
    pub local_device_id: String,
    pub network_enabled: bool,
    pub auto_bridge_remote_text: bool,
    pub peers: Vec<SystemPeer>,
    pub local_clipboard: ClipboardSnapshot,
    pub latest_remote_clipboard: Option<ClipboardSnapshot>,
}

#[derive(Clone)]
pub struct SystemPeer {
    pub node_id: String,
    pub device_id: String,
    pub ip: String,
    pub status: SystemPeerStatus,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SystemPeerStatus {
    Connected,
    Transferring,
}

#[derive(Clone)]
pub struct ClipboardSnapshot {
    pub origin: ClipboardOrigin,
    pub device_id: String,
    pub updated_at_order: u64,
    pub captured_at_label: String,
    pub content: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ClipboardOrigin {
    Local,
    Remote,
}

#[derive(Clone)]
pub struct PendingFileDecision {
    pub file_name: String,
    pub peer_label: String,
    pub size_label: String,
}

#[derive(Clone)]
pub struct ActivityItem {
    pub time_label: String,
    pub kind: String,
    pub title: String,
    pub detail: String,
}

#[derive(Clone)]
pub struct TransferItem {
    pub file_name: String,
    pub progress: f32,
    pub bytes_label: String,
    pub speed_label: String,
    pub eta_label: String,
}
