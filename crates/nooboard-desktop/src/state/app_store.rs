#[derive(Clone)]
pub struct SharedState {
    pub app: AppStore,
}

impl SharedState {
    pub fn demo() -> Self {
        let transfer_rail_items = vec![
            TransferRailItem::awaiting_review(
                "awaiting-report-quarterly",
                "report-quarterly-final-v7-with-annotations.pdf",
                "14.2 MB",
                "node-alpha",
                "12:18",
            ),
            TransferRailItem::awaiting_review(
                "awaiting-debug-bundle",
                "archive-project-handshake-debug-bundle.zip",
                "88.5 MB",
                "node-beta",
                "12:21",
            ),
            TransferRailItem::awaiting_review(
                "transfer-deployment-bundle",
                "deployment-bundle-2026-03-04.tar.zst",
                "150 MB",
                "render-node",
                "12:12",
            )
            .start_transfer(0.61, "3.2 MB/s", "12:14", "3m 18s", "ETA 18s"),
            TransferRailItem::awaiting_review(
                "transfer-capture-sequence",
                "capture-sequence-raw-frames.zip",
                "25 MB",
                "mbp-lab",
                "12:23",
            )
            .start_transfer(0.24, "1.1 MB/s", "12:24", "46s", "ETA 14s"),
            TransferRailItem::awaiting_review(
                "completed-evidence-pack",
                "evidence-pack-alpha-02.zip",
                "34.2 MB",
                "node-alpha",
                "12:01",
            )
            .start_transfer(1.0, "6.8 MB/s", "12:02", "31s", "ETA 0s")
            .complete_transfer("12:03", "31s"),
            TransferRailItem::awaiting_review(
                "completed-screen-capture",
                "screen-capture-ux-pass.mov",
                "72.8 MB",
                "node-gamma",
                "11:56",
            )
            .start_transfer(1.0, "5.2 MB/s", "11:57", "1m 06s", "ETA 0s")
            .complete_transfer("11:58", "1m 06s"),
        ];

        Self {
            app: AppStore {
                online_peers: 3,
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
                    },
                    ActivityItem {
                        time_label: "12:08".into(),
                        kind: "ConnectionError".into(),
                        title: "192.168.1.9 rejected handshake during capability negotiation".into(),
                    },
                    ActivityItem {
                        time_label: "12:10".into(),
                        kind: "TransferFinished".into(),
                        title: "report-quarterly-final-v7-with-annotations.pdf downloaded".into(),
                    },
                    ActivityItem {
                        time_label: "12:18".into(),
                        kind: "ReviewQueued".into(),
                        title: "archive-project-handshake-debug-bundle.zip queued for operator review".into(),
                    },
                    ActivityItem {
                        time_label: "12:24".into(),
                        kind: "TransferStarted".into(),
                        title: "capture-sequence-raw-frames.zip started transferring from mbp-lab".into(),
                    },
                ],
                recent_history: vec![
                    "alpha release checklist updated after handoff".into(),
                    "build finished".into(),
                    "ssh key copied".into(),
                    "remote message from node-gamma with a long troubleshooting summary".into(),
                    "deploy command with temporary feature flags and validation notes".into(),
                ],
                transfer_rail_items,
            },
        }
    }
}

#[derive(Clone)]
pub struct AppStore {
    pub online_peers: usize,
    pub system_core: SystemCoreStore,
    pub pending_files: Vec<PendingFileDecision>,
    pub recent_activity: Vec<ActivityItem>,
    pub recent_history: Vec<String>,
    pub transfer_rail_items: Vec<TransferRailItem>,
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
}

#[derive(Clone)]
pub struct TransferRailItem {
    pub id: String,
    pub file_name: String,
    pub size_label: String,
    pub source_device: String,
    pub status: TransferRailStatus,
}

impl TransferRailItem {
    pub fn awaiting_review(
        id: impl Into<String>,
        file_name: impl Into<String>,
        size_label: impl Into<String>,
        source_device: impl Into<String>,
        queued_at_label: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            file_name: file_name.into(),
            size_label: size_label.into(),
            source_device: source_device.into(),
            status: TransferRailStatus::AwaitingReview {
                queued_at_label: queued_at_label.into(),
            },
        }
    }

    pub fn start_transfer(
        self,
        progress: f32,
        speed_label: impl Into<String>,
        started_at_label: impl Into<String>,
        elapsed_label: impl Into<String>,
        eta_label: impl Into<String>,
    ) -> Self {
        match self.status {
            TransferRailStatus::AwaitingReview { .. } => Self {
                id: self.id,
                file_name: self.file_name,
                size_label: self.size_label,
                source_device: self.source_device,
                status: TransferRailStatus::InProgress {
                    progress,
                    speed_label: speed_label.into(),
                    started_at_label: started_at_label.into(),
                    elapsed_label: elapsed_label.into(),
                    eta_label: eta_label.into(),
                },
            },
            _ => panic!("transfer rail item can only enter progress from awaiting review"),
        }
    }

    pub fn complete_transfer(
        self,
        completed_at_label: impl Into<String>,
        duration_label: impl Into<String>,
    ) -> Self {
        match self.status {
            TransferRailStatus::InProgress { .. } => Self {
                id: self.id,
                file_name: self.file_name,
                size_label: self.size_label,
                source_device: self.source_device,
                status: TransferRailStatus::Completed {
                    completed_at_label: completed_at_label.into(),
                    duration_label: duration_label.into(),
                },
            },
            _ => panic!("transfer rail item can only complete from in-progress"),
        }
    }

    pub fn stage(&self) -> TransferRailStage {
        match &self.status {
            TransferRailStatus::AwaitingReview { .. } => TransferRailStage::AwaitingReview,
            TransferRailStatus::InProgress { .. } => TransferRailStage::InProgress,
            TransferRailStatus::Completed { .. } => TransferRailStage::Completed,
        }
    }

    pub fn is_awaiting_review(&self) -> bool {
        self.stage() == TransferRailStage::AwaitingReview
    }

    pub fn is_in_progress(&self) -> bool {
        self.stage() == TransferRailStage::InProgress
    }

    pub fn is_completed(&self) -> bool {
        self.stage() == TransferRailStage::Completed
    }
}

#[derive(Clone)]
pub enum TransferRailStatus {
    AwaitingReview {
        queued_at_label: String,
    },
    InProgress {
        progress: f32,
        speed_label: String,
        started_at_label: String,
        elapsed_label: String,
        eta_label: String,
    },
    Completed {
        completed_at_label: String,
        duration_label: String,
    },
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TransferRailStage {
    AwaitingReview,
    InProgress,
    Completed,
}
