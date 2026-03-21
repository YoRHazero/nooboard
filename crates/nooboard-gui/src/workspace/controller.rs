use std::{collections::VecDeque, sync::Arc};

use gpui::{Context, Entity};
use nooboard_core::{
    BootstrapLaunch, ClipboardRecord, NooboardCore, WorkspaceEvent, WorkspaceSnapshot,
};

use super::{
    LaunchHandle,
    core_bridge::{CoreBridge, CoreBridgeBoot},
    recent_activity::{
        RecentActivityItem, RecentActivityKind, recent_activity_from_network_status,
        recent_activity_from_workspace_event,
    },
    route::WorkspaceRoute,
    runtime_state::{WorkspaceBridgeState, WorkspaceLoadState},
    subscriptions::{spawn_event_bridge, spawn_state_bridge},
};

const RECENT_ACTIVITY_CAPACITY: usize = 64;

pub struct WorkspaceController {
    launch: BootstrapLaunch,
    route: WorkspaceRoute,
    load_state: WorkspaceLoadState,
    bridge: Option<CoreBridge>,
    snapshot: Option<WorkspaceSnapshot>,
    latest_committed_record: Option<ClipboardRecord>,
    recent_activity: VecDeque<RecentActivityItem>,
    bridge_state: WorkspaceBridgeState,
}

impl WorkspaceController {
    pub fn new(launch: LaunchHandle) -> Self {
        let LaunchHandle::Ready(launch) = launch;
        Self {
            launch,
            route: WorkspaceRoute::Home,
            load_state: WorkspaceLoadState::Loading,
            bridge: None,
            snapshot: None,
            latest_committed_record: None,
            recent_activity: VecDeque::with_capacity(RECENT_ACTIVITY_CAPACITY),
            bridge_state: WorkspaceBridgeState::default(),
        }
    }

    pub fn initialize<T: 'static>(
        controller: &Entity<Self>,
        launch: LaunchHandle,
        cx: &Context<T>,
    ) {
        let controller = controller.downgrade();

        cx.spawn(async move |_, cx| {
            let LaunchHandle::Ready(launch) = launch;
            match CoreBridge::launch(&launch).await {
                Ok(CoreBridgeBoot {
                    bridge,
                    snapshot,
                    latest_committed_record,
                    subscriptions,
                }) => {
                    let state_bridge = bridge.clone();
                    let event_bridge = bridge.clone();

                    let _ = controller.update(cx, |this, cx| {
                        this.attach_bridge(bridge, snapshot, latest_committed_record);
                        cx.notify();
                    });

                    spawn_state_bridge(controller.clone(), state_bridge, subscriptions.state, cx);
                    spawn_event_bridge(controller.clone(), event_bridge, subscriptions.events, cx);
                }
                Err(error) => {
                    let _ = controller.update(cx, |this, cx| {
                        this.mark_launch_failed(error.to_string());
                        cx.notify();
                    });
                }
            }

            Ok::<_, anyhow::Error>(())
        })
        .detach();
    }

    pub fn launch(&self) -> &BootstrapLaunch {
        &self.launch
    }

    pub fn route(&self) -> WorkspaceRoute {
        self.route
    }

    pub fn set_route(&mut self, route: WorkspaceRoute) {
        self.route = route;
    }

    pub fn load_state(&self) -> &WorkspaceLoadState {
        &self.load_state
    }

    pub fn bridge_state(&self) -> &WorkspaceBridgeState {
        &self.bridge_state
    }

    pub fn snapshot(&self) -> Option<&WorkspaceSnapshot> {
        self.snapshot.as_ref()
    }

    pub fn latest_committed_record(&self) -> Option<&ClipboardRecord> {
        self.latest_committed_record.as_ref()
    }

    pub fn recent_activity(&self) -> &VecDeque<RecentActivityItem> {
        &self.recent_activity
    }

    pub fn core(&self) -> Option<Arc<NooboardCore>> {
        self.bridge.as_ref().map(CoreBridge::core)
    }

    pub fn attach_bridge(
        &mut self,
        bridge: CoreBridge,
        snapshot: WorkspaceSnapshot,
        latest_committed_record: Option<ClipboardRecord>,
    ) {
        self.load_state = WorkspaceLoadState::Ready;
        self.bridge = Some(bridge);
        self.snapshot = Some(snapshot);
        self.latest_committed_record = latest_committed_record;
        self.bridge_state = WorkspaceBridgeState::default();
    }

    pub fn apply_snapshot(&mut self, snapshot: WorkspaceSnapshot) {
        let previous_status = self
            .snapshot
            .as_ref()
            .map(|current| current.network.status.clone());
        let next_status = snapshot.network.status.clone();
        self.snapshot = Some(snapshot);

        if previous_status.as_ref() != Some(&next_status) {
            self.push_recent_activity(recent_activity_from_network_status(&next_status));
        }
    }

    pub fn replace_latest_committed_record(&mut self, record: Option<ClipboardRecord>) {
        self.latest_committed_record = record;
    }

    pub fn apply_event(&mut self, event: WorkspaceEvent) {
        if let Some(activity) = recent_activity_from_workspace_event(&event) {
            self.push_recent_activity(activity);
        }
    }

    pub fn record_bridge_warning(&mut self, message: String) {
        self.bridge_state.last_error = Some(message.clone());
        self.push_recent_activity(RecentActivityItem::new(RecentActivityKind::GuiWarning {
            message,
        }));
    }

    pub fn mark_state_stream_closed(&mut self, reason: String) {
        self.bridge_state.state_stream_open = false;
        self.bridge_state.last_error = Some(reason.clone());
        self.push_recent_activity(RecentActivityItem::new(RecentActivityKind::GuiError {
            message: format!("Live sync updates stopped: {reason}"),
        }));
    }

    pub fn mark_event_stream_closed(&mut self, reason: String) {
        self.bridge_state.event_stream_open = false;
        self.bridge_state.last_error = Some(reason.clone());
        self.push_recent_activity(RecentActivityItem::new(RecentActivityKind::GuiError {
            message: format!("Live activity updates stopped: {reason}"),
        }));
    }

    pub fn mark_launch_failed(&mut self, message: String) {
        self.load_state = WorkspaceLoadState::Failed(message.clone());
        self.bridge_state.last_error = Some(message.clone());
        self.push_recent_activity(RecentActivityItem::new(RecentActivityKind::GuiError {
            message,
        }));
    }

    fn push_recent_activity(&mut self, item: RecentActivityItem) {
        self.recent_activity.push_front(item);
        while self.recent_activity.len() > RECENT_ACTIVITY_CAPACITY {
            let _ = self.recent_activity.pop_back();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use nooboard_core::{
        BootstrapLaunch, BootstrapMode, ClipboardSettings, ClipboardState, ConnectionSettings,
        LocalConnectionInfo, NetworkSettings, NetworkSnapshot, NetworkStatus, NoobId,
        StorageSettings, TransferSettings, TransfersSnapshot, WorkspaceIdentity, WorkspaceSettings,
        WorkspaceSnapshot,
    };

    use super::*;

    #[test]
    fn recent_activity_keeps_newest_entries_with_fixed_capacity() {
        let mut controller = WorkspaceController::new(LaunchHandle::Ready(sample_launch()));

        for index in 0..(RECENT_ACTIVITY_CAPACITY + 4) {
            controller.record_bridge_warning(format!("warning-{index}"));
        }

        assert_eq!(controller.recent_activity().len(), RECENT_ACTIVITY_CAPACITY);
        assert!(matches!(
            controller.recent_activity().front().map(|item| &item.kind),
            Some(RecentActivityKind::GuiWarning { .. })
        ));
    }

    #[test]
    fn network_status_transition_records_recent_activity() {
        let mut controller = WorkspaceController::new(LaunchHandle::Ready(sample_launch()));
        controller.snapshot = Some(sample_snapshot(NetworkStatus::Stopped));

        controller.apply_snapshot(sample_snapshot(NetworkStatus::Running));

        assert!(matches!(
            controller.recent_activity().front().map(|item| &item.kind),
            Some(RecentActivityKind::NetworkRunning)
        ));
    }

    fn sample_launch() -> BootstrapLaunch {
        BootstrapLaunch {
            mode: BootstrapMode::UserDefault,
            config_path: PathBuf::from("config.toml"),
        }
    }

    fn sample_snapshot(status: NetworkStatus) -> WorkspaceSnapshot {
        WorkspaceSnapshot {
            revision: 1,
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
