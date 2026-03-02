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
                today_history_count: 18,
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
    pub pending_files: Vec<PendingFileDecision>,
    pub recent_activity: Vec<ActivityItem>,
    pub transfers: Vec<TransferItem>,
    pub recent_history: Vec<String>,
    pub today_history_count: usize,
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
