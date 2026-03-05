use uuid::Uuid;

use super::clipboard::{ClipboardHistoryPage, ClipboardStore, ClipboardTarget, ClipboardTextItem};
use super::transfer::TransferItem;

#[derive(Clone)]
pub struct SharedState {
    pub app: AppStore,
}

impl SharedState {
    pub fn demo() -> Self {
        let transfer_items = vec![
            TransferItem::awaiting_review(
                "awaiting-report-quarterly",
                "report-quarterly-final-v7-with-annotations.pdf",
                "14.2 MB",
                "node-alpha",
                "12:18",
            ),
            TransferItem::awaiting_review(
                "awaiting-debug-bundle",
                "archive-project-handshake-debug-bundle.zip",
                "88.5 MB",
                "node-beta",
                "12:21",
            ),
            TransferItem::awaiting_review(
                "transfer-deployment-bundle",
                "deployment-bundle-2026-03-04.tar.zst",
                "150 MB",
                "render-node",
                "12:12",
            )
            .start_transfer(0.61, "3.2 MB/s", "12:14", "3m 18s", "ETA 18s"),
            TransferItem::awaiting_review(
                "transfer-capture-sequence",
                "capture-sequence-raw-frames.zip",
                "25 MB",
                "mbp-lab",
                "12:23",
            )
            .start_transfer(0.24, "1.1 MB/s", "12:24", "46s", "ETA 14s"),
            TransferItem::awaiting_review(
                "complete-evidence-pack",
                "evidence-pack-alpha-02.zip",
                "34.2 MB",
                "node-alpha",
                "12:01",
            )
            .start_transfer(1.0, "6.8 MB/s", "12:02", "31s", "ETA 0s")
            .complete_transfer("12:03", "31s"),
            TransferItem::awaiting_review(
                "complete-screen-capture",
                "screen-capture-ux-pass.mov",
                "72.8 MB",
                "node-gamma",
                "11:56",
            )
            .start_transfer(1.0, "5.2 MB/s", "11:57", "1m 06s", "ETA 0s")
            .complete_transfer("11:58", "1m 06s"),
        ];

        let clipboard_targets = vec![
            ClipboardTarget::connected("b71d8bb8-e35b-4e2e-9278-6d99ab77b6cf", "mbp-lab"),
            ClipboardTarget::connected("3177c7f4-d664-4a5a-823e-8e5a962f34f1", "render-node"),
            ClipboardTarget::connected("8d9c9bc0-b070-4b7e-89f9-4712929b89b5", "mbp-lab"),
            ClipboardTarget::offline("9ab31b96-f61d-4f08-b2e5-c2b1d7e30b60", "node-alpha"),
        ];

        let local_live = ClipboardTextItem::local_live(
            Uuid::from_u128(0x018fbf9ad8ea7c13a2f00e09d8300001),
            "desk-01",
            2026_03_03_12_14_08,
            "2026-03-03 12:14:08",
            "cargo test -p nooboard-desktop --workspace",
        );
        let remote_live = ClipboardTextItem::remote_live(
            Uuid::from_u128(0x018fbf9ad8ea7c13a2f00e09d8300002),
            "render-node",
            2026_03_03_12_16_27,
            "2026-03-03 12:16:27",
            "handoff ready: transfer lane 02 is draining and db write is still pending",
        );

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
                },
                clipboard: ClipboardStore {
                    targets: clipboard_targets,
                    default_selected_target_node_ids: vec![
                        "b71d8bb8-e35b-4e2e-9278-6d99ab77b6cf".into(),
                        "3177c7f4-d664-4a5a-823e-8e5a962f34f1".into(),
                    ],
                    local_live,
                    latest_remote_live: Some(remote_live),
                    history_pages: vec![
                        ClipboardHistoryPage::new(vec![
                            ClipboardTextItem::remote_history(
                                Uuid::from_u128(0x018fbf9ad8ea7c13a2f00e09d8300010),
                                "node-gamma",
                                2026_03_03_12_05_44,
                                "2026-03-03 12:05:44",
                                "remote message from node-gamma with a long troubleshooting summary",
                            ),
                            ClipboardTextItem::local_history(
                                Uuid::from_u128(0x018fbf9ad8ea7c13a2f00e09d8300011),
                                "desk-01",
                                2026_03_03_11_48_19,
                                "2026-03-03 11:48:19",
                                "deploy command with temporary feature flags and validation notes",
                            ),
                            ClipboardTextItem::remote_history(
                                Uuid::from_u128(0x018fbf9ad8ea7c13a2f00e09d8300012),
                                "render-node",
                                2026_03_03_11_42_07,
                                "2026-03-03 11:42:07",
                                "alpha release checklist updated after handoff",
                            ),
                        ]),
                        ClipboardHistoryPage::new(vec![
                            ClipboardTextItem::local_history(
                                Uuid::from_u128(0x018fbf9ad8ea7c13a2f00e09d8300013),
                                "desk-01",
                                2026_03_03_10_51_30,
                                "2026-03-03 10:51:30",
                                "ssh key copied",
                            ),
                            ClipboardTextItem::local_history(
                                Uuid::from_u128(0x018fbf9ad8ea7c13a2f00e09d8300014),
                                "desk-01",
                                2026_03_03_10_18_02,
                                "2026-03-03 10:18:02",
                                "build finished",
                            ),
                            ClipboardTextItem::remote_history(
                                Uuid::from_u128(0x018fbf9ad8ea7c13a2f00e09d8300015),
                                "mbp-lab",
                                2026_03_03_09_57_11,
                                "2026-03-03 09:57:11",
                                "copy the migration notes before restarting the app runtime",
                            ),
                        ]),
                    ],
                },
                recent_activity: vec![
                    ActivityItem {
                        time_label: "12:03".into(),
                        kind: "TextReceived".into(),
                        title: "device-a sent \"alpha release checklist updated after handoff\""
                            .into(),
                    },
                    ActivityItem {
                        time_label: "12:08".into(),
                        kind: "ConnectionError".into(),
                        title: "192.168.1.9 rejected handshake during capability negotiation"
                            .into(),
                    },
                    ActivityItem {
                        time_label: "12:10".into(),
                        kind: "TransferFinished".into(),
                        title: "report-quarterly-final-v7-with-annotations.pdf downloaded".into(),
                    },
                    ActivityItem {
                        time_label: "12:18".into(),
                        kind: "ReviewQueued".into(),
                        title:
                            "archive-project-handshake-debug-bundle.zip queued for operator review"
                                .into(),
                    },
                    ActivityItem {
                        time_label: "12:24".into(),
                        kind: "TransferStarted".into(),
                        title: "capture-sequence-raw-frames.zip started transferring from mbp-lab"
                            .into(),
                    },
                ],
                transfer_items,
            },
        }
    }
}

#[derive(Clone)]
pub struct AppStore {
    pub online_peers: usize,
    pub system_core: SystemCoreStore,
    pub clipboard: ClipboardStore,
    pub recent_activity: Vec<ActivityItem>,
    pub transfer_items: Vec<TransferItem>,
}

#[derive(Clone)]
pub struct SystemCoreStore {
    pub local_device_id: String,
    pub network_enabled: bool,
    pub auto_bridge_remote_text: bool,
    pub peers: Vec<SystemPeer>,
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
pub struct ActivityItem {
    pub time_label: String,
    pub kind: String,
    pub title: String,
}
