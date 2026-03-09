use std::collections::VecDeque;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context as _, Result};
use gpui::{App, AppContext, AsyncApp, Context, Entity, Global, WeakEntity};
use nooboard_app::{
    AppEvent, AppState, ClipboardRecord, ClipboardRecordSource, DesktopAppService,
    DesktopAppServiceImpl, EventId, EventSubscription, NoobId, StateSubscription, SyncActualStatus,
    TransferId, TransferOutcome,
};
use tokio::runtime::{Builder, Runtime};

const RECENT_ACTIVITY_CAPACITY: usize = 64;

#[derive(Clone)]
pub struct DesktopLiveApp {
    runtime: Arc<Runtime>,
    service: Arc<DesktopAppServiceImpl>,
    store: Entity<LiveAppStore>,
    config_path: PathBuf,
}

impl Global for DesktopLiveApp {}

#[allow(dead_code)]
impl DesktopLiveApp {
    pub fn runtime(&self) -> Arc<Runtime> {
        self.runtime.clone()
    }

    pub fn service(&self) -> Arc<DesktopAppServiceImpl> {
        self.service.clone()
    }

    pub fn store(&self) -> Entity<LiveAppStore> {
        self.store.clone()
    }

    pub fn config_path(&self) -> &Path {
        self.config_path.as_path()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecentActivitySeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecentActivityKind {
    ClipboardCommitted {
        event_id: EventId,
        source: ClipboardRecordSource,
    },
    IncomingTransferOffered {
        transfer_id: TransferId,
    },
    TransferCompleted {
        transfer_id: TransferId,
        outcome: TransferOutcome,
    },
    PeerConnectionError {
        peer_noob_id: Option<NoobId>,
        addr: Option<SocketAddr>,
        error: String,
    },
    SyncStarting,
    SyncRunning,
    SyncStopped,
    SyncDisabledBySettings,
    SyncError {
        message: String,
    },
    DesktopWarning {
        message: String,
    },
    DesktopError {
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecentActivityItem {
    pub observed_at_ms: i64,
    pub severity: RecentActivitySeverity,
    pub kind: RecentActivityKind,
}

impl RecentActivityItem {
    fn new(kind: RecentActivityKind) -> Self {
        Self {
            observed_at_ms: now_millis(),
            severity: activity_severity(&kind),
            kind,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DesktopLocalPreferences {
    pub auto_adopt_remote_clipboard: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopLiveBridgeState {
    pub state_stream_open: bool,
    pub event_stream_open: bool,
    pub last_error: Option<String>,
}

impl Default for DesktopLiveBridgeState {
    fn default() -> Self {
        Self {
            state_stream_open: true,
            event_stream_open: true,
            last_error: None,
        }
    }
}

pub struct LiveAppStore {
    config_path: PathBuf,
    app_state: AppState,
    latest_committed_record: Option<ClipboardRecord>,
    recent_activity: VecDeque<RecentActivityItem>,
    local_preferences: DesktopLocalPreferences,
    bridge: DesktopLiveBridgeState,
}

#[allow(dead_code)]
impl LiveAppStore {
    fn new(
        config_path: PathBuf,
        app_state: AppState,
        latest_committed_record: Option<ClipboardRecord>,
    ) -> Self {
        Self {
            config_path,
            app_state,
            latest_committed_record,
            recent_activity: VecDeque::with_capacity(RECENT_ACTIVITY_CAPACITY),
            local_preferences: DesktopLocalPreferences::default(),
            bridge: DesktopLiveBridgeState::default(),
        }
    }

    pub fn config_path(&self) -> &Path {
        self.config_path.as_path()
    }

    pub fn app_state(&self) -> &AppState {
        &self.app_state
    }

    pub fn latest_committed_record(&self) -> Option<&ClipboardRecord> {
        self.latest_committed_record.as_ref()
    }

    pub fn recent_activity(&self) -> &VecDeque<RecentActivityItem> {
        &self.recent_activity
    }

    pub fn local_preferences(&self) -> &DesktopLocalPreferences {
        &self.local_preferences
    }

    pub fn bridge(&self) -> &DesktopLiveBridgeState {
        &self.bridge
    }

    pub fn set_auto_adopt_remote_clipboard(&mut self, enabled: bool, cx: &mut Context<Self>) {
        if self.local_preferences.auto_adopt_remote_clipboard == enabled {
            return;
        }

        self.local_preferences.auto_adopt_remote_clipboard = enabled;
        cx.notify();
    }

    fn apply_state_snapshot(&mut self, app_state: AppState) {
        self.app_state = app_state;
    }

    fn replace_latest_committed_record(&mut self, record: Option<ClipboardRecord>) {
        self.latest_committed_record = record;
    }

    fn push_recent_activity(&mut self, item: RecentActivityItem) {
        self.recent_activity.push_front(item);
        while self.recent_activity.len() > RECENT_ACTIVITY_CAPACITY {
            let _ = self.recent_activity.pop_back();
        }
    }

    fn mark_state_stream_closed(&mut self, reason: String) {
        self.bridge.state_stream_open = false;
        self.bridge.last_error = Some(reason.clone());
        self.push_recent_activity(RecentActivityItem::new(RecentActivityKind::DesktopError {
            message: format!("desktop state stream closed: {reason}"),
        }));
    }

    fn mark_event_stream_closed(&mut self, reason: String) {
        self.bridge.event_stream_open = false;
        self.bridge.last_error = Some(reason.clone());
        self.push_recent_activity(RecentActivityItem::new(RecentActivityKind::DesktopError {
            message: format!("desktop event stream closed: {reason}"),
        }));
    }

    fn record_bridge_error(&mut self, error: String) {
        self.bridge.last_error = Some(error.clone());
        self.push_recent_activity(RecentActivityItem::new(
            RecentActivityKind::DesktopWarning { message: error },
        ));
    }
}

pub fn install_desktop_live_app(
    config_path: impl AsRef<Path>,
    cx: &mut App,
) -> Result<DesktopLiveApp> {
    let config_path = config_path.as_ref().to_path_buf();
    let runtime = Arc::new(build_runtime()?);
    let service = {
        let _guard = runtime.enter();
        Arc::new(
            DesktopAppServiceImpl::new(&config_path)
                .with_context(|| format!("bootstrap app service from {}", config_path.display()))?,
        )
    };
    let (state_subscription, event_subscription, initial_state, initial_record) = runtime
        .block_on(async {
            let state_subscription = service.subscribe_state().await?;
            let event_subscription = service.subscribe_events().await?;
            let initial_state = state_subscription.latest().clone();
            let initial_record =
                fetch_latest_committed_record(service.as_ref(), &initial_state).await?;
            Ok::<_, anyhow::Error>((
                state_subscription,
                event_subscription,
                initial_state,
                initial_record,
            ))
        })?;

    let store = cx.new(|_| LiveAppStore::new(config_path.clone(), initial_state, initial_record));
    let live_app = DesktopLiveApp {
        runtime,
        service: service.clone(),
        store: store.clone(),
        config_path,
    };

    cx.set_global(live_app.clone());
    spawn_state_bridge(store.downgrade(), service.clone(), state_subscription, cx);
    spawn_event_bridge(store.downgrade(), service, event_subscription, cx);

    Ok(live_app)
}

async fn fetch_latest_committed_record(
    service: &DesktopAppServiceImpl,
    state: &AppState,
) -> Result<Option<ClipboardRecord>> {
    match state.clipboard.latest_committed_event_id {
        Some(event_id) => Ok(Some(service.get_clipboard_record(event_id).await?)),
        None => Ok(None),
    }
}

fn activity_severity(kind: &RecentActivityKind) -> RecentActivitySeverity {
    match kind {
        RecentActivityKind::ClipboardCommitted { .. }
        | RecentActivityKind::IncomingTransferOffered { .. }
        | RecentActivityKind::TransferCompleted { .. }
        | RecentActivityKind::SyncStarting
        | RecentActivityKind::SyncRunning
        | RecentActivityKind::SyncStopped => RecentActivitySeverity::Info,
        RecentActivityKind::PeerConnectionError { .. }
        | RecentActivityKind::SyncDisabledBySettings
        | RecentActivityKind::DesktopWarning { .. } => RecentActivitySeverity::Warning,
        RecentActivityKind::SyncError { .. } | RecentActivityKind::DesktopError { .. } => {
            RecentActivitySeverity::Error
        }
    }
}

fn recent_activity_from_app_event(event: &AppEvent) -> Option<RecentActivityItem> {
    let kind = match event {
        AppEvent::ClipboardCommitted { event_id, source } => {
            RecentActivityKind::ClipboardCommitted {
                event_id: *event_id,
                source: source.clone(),
            }
        }
        AppEvent::IncomingTransferOffered { transfer_id } => {
            RecentActivityKind::IncomingTransferOffered {
                transfer_id: transfer_id.clone(),
            }
        }
        AppEvent::TransferUpdated { .. } => return None,
        AppEvent::TransferCompleted {
            transfer_id,
            outcome,
        } => RecentActivityKind::TransferCompleted {
            transfer_id: transfer_id.clone(),
            outcome: outcome.clone(),
        },
        AppEvent::PeerConnectionError {
            peer_noob_id,
            addr,
            error,
        } => RecentActivityKind::PeerConnectionError {
            peer_noob_id: peer_noob_id.clone(),
            addr: *addr,
            error: error.clone(),
        },
    };

    Some(RecentActivityItem::new(kind))
}

fn recent_activity_from_sync_state(state: &AppState) -> Option<RecentActivityItem> {
    let kind = match &state.sync.actual {
        SyncActualStatus::Starting => RecentActivityKind::SyncStarting,
        SyncActualStatus::Running => RecentActivityKind::SyncRunning,
        SyncActualStatus::Stopped if state.settings.network.network_enabled => {
            RecentActivityKind::SyncStopped
        }
        SyncActualStatus::Stopped => return None,
        SyncActualStatus::Disabled => RecentActivityKind::SyncDisabledBySettings,
        SyncActualStatus::Error(message) => RecentActivityKind::SyncError {
            message: message.clone(),
        },
    };

    Some(RecentActivityItem::new(kind))
}

fn spawn_state_bridge(
    store: WeakEntity<LiveAppStore>,
    service: Arc<DesktopAppServiceImpl>,
    mut subscription: StateSubscription,
    cx: &mut App,
) {
    cx.spawn(async move |cx: &mut AsyncApp| {
        let mut latest_record_event_id = subscription.latest().clipboard.latest_committed_event_id;
        let mut latest_sync_actual = subscription.latest().sync.actual.clone();

        loop {
            let next_state = match subscription.recv().await {
                Ok(state) => state,
                Err(error) => {
                    let _ = store.update(cx, |store, cx| {
                        store.mark_state_stream_closed(error.to_string());
                        cx.notify();
                    });
                    break;
                }
            };

            let next_record_event_id = next_state.clipboard.latest_committed_event_id;
            let refresh_latest_record = next_record_event_id != latest_record_event_id;
            latest_record_event_id = next_record_event_id;
            let sync_activity = if next_state.sync.actual != latest_sync_actual {
                recent_activity_from_sync_state(&next_state)
            } else {
                None
            };
            latest_sync_actual = next_state.sync.actual.clone();

            let _ = store.update(cx, |store, cx| {
                store.apply_state_snapshot(next_state.clone());
                if let Some(activity) = sync_activity {
                    store.push_recent_activity(activity);
                }
                cx.notify();
            });

            if !refresh_latest_record {
                continue;
            }

            match fetch_latest_committed_record(service.as_ref(), &next_state).await {
                Ok(record) => {
                    let _ = store.update(cx, |store, cx| {
                        store.replace_latest_committed_record(record);
                        cx.notify();
                    });
                }
                Err(error) => {
                    let _ = store.update(cx, |store, cx| {
                        store.record_bridge_error(format!(
                            "failed to refresh latest committed record: {error}"
                        ));
                        cx.notify();
                    });
                }
            }
        }

        Ok::<_, anyhow::Error>(())
    })
    .detach();
}

fn spawn_event_bridge(
    store: WeakEntity<LiveAppStore>,
    service: Arc<DesktopAppServiceImpl>,
    mut subscription: EventSubscription,
    cx: &mut App,
) {
    cx.spawn(async move |cx: &mut AsyncApp| {
        loop {
            let event = match subscription.recv().await {
                Ok(event) => event,
                Err(error) => {
                    let _ = store.update(cx, |store, cx| {
                        store.mark_event_stream_closed(error.to_string());
                        cx.notify();
                    });
                    break;
                }
            };

            let auto_adopt_remote_clipboard = store
                .update(cx, |store, cx| {
                    let should_auto_adopt =
                        matches!(
                            event,
                            AppEvent::ClipboardCommitted {
                                source: ClipboardRecordSource::RemoteSync,
                                ..
                            }
                        ) && store.local_preferences.auto_adopt_remote_clipboard;

                    if let Some(activity) = recent_activity_from_app_event(&event) {
                        store.push_recent_activity(activity);
                    }
                    cx.notify();
                    should_auto_adopt
                })
                .unwrap_or(false);

            if !auto_adopt_remote_clipboard {
                continue;
            }

            let AppEvent::ClipboardCommitted { event_id, .. } = event else {
                continue;
            };

            if let Err(error) = service.adopt_clipboard_record(event_id).await {
                let _ = store.update(cx, |store, cx| {
                    store.record_bridge_error(format!(
                        "failed to auto-adopt remote clipboard record {event_id}: {error}"
                    ));
                    cx.notify();
                });
            }
        }

        Ok::<_, anyhow::Error>(())
    })
    .detach();
}

fn build_runtime() -> Result<Runtime> {
    Builder::new_multi_thread()
        .worker_threads(2)
        .thread_name("nooboard-desktop")
        .enable_all()
        .build()
        .context("create desktop tokio runtime")
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
        ClipboardState, LocalIdentity, NetworkSettings, NoobId, PeersState, SettingsState,
        StorageSettings, SyncActualStatus, SyncDesiredState, SyncState, TransferSettings,
        TransfersState,
    };

    use super::*;

    #[test]
    fn recent_activity_keeps_newest_entries_with_fixed_capacity() {
        let mut store = LiveAppStore::new(PathBuf::from("config.toml"), sample_state(), None);

        for index in 0..(RECENT_ACTIVITY_CAPACITY + 4) {
            store.push_recent_activity(RecentActivityItem {
                observed_at_ms: index as i64,
                severity: RecentActivitySeverity::Warning,
                kind: RecentActivityKind::DesktopWarning {
                    message: format!("warning-{index}"),
                },
            });
        }

        assert_eq!(store.recent_activity().len(), RECENT_ACTIVITY_CAPACITY);
        assert_eq!(store.recent_activity().front().unwrap().observed_at_ms, 67);
        assert_eq!(store.recent_activity().back().unwrap().observed_at_ms, 4);
    }

    #[test]
    fn sync_error_transitions_map_to_error_activity() {
        let mut state = sample_state();
        state.sync.actual = SyncActualStatus::Error("engine died".to_string());

        let activity = recent_activity_from_sync_state(&state).expect("sync error should map");
        assert_eq!(activity.severity, RecentActivitySeverity::Error);
        assert_eq!(
            activity.kind,
            RecentActivityKind::SyncError {
                message: "engine died".to_string(),
            }
        );
    }

    #[test]
    fn bridge_error_state_tracks_closed_streams() {
        let mut store = LiveAppStore::new(PathBuf::from("config.toml"), sample_state(), None);

        store.mark_state_stream_closed("state closed".to_string());
        assert!(!store.bridge().state_stream_open);
        assert!(store.bridge().event_stream_open);
        assert_eq!(store.bridge().last_error.as_deref(), Some("state closed"));
        assert_eq!(
            store.recent_activity().front().unwrap().severity,
            RecentActivitySeverity::Error
        );

        store.mark_event_stream_closed("event closed".to_string());
        assert!(!store.bridge().event_stream_open);
        assert_eq!(store.bridge().last_error.as_deref(), Some("event closed"));
        assert_eq!(store.recent_activity().len(), 2);
    }

    fn sample_state() -> AppState {
        AppState {
            revision: 1,
            identity: LocalIdentity {
                noob_id: NoobId::new("local"),
                device_id: "desk".to_string(),
            },
            sync: SyncState {
                desired: SyncDesiredState::Stopped,
                actual: SyncActualStatus::Stopped,
            },
            peers: PeersState::default(),
            clipboard: ClipboardState::default(),
            transfers: TransfersState::default(),
            settings: SettingsState {
                network: NetworkSettings {
                    network_enabled: true,
                    mdns_enabled: true,
                    manual_peers: Vec::new(),
                },
                storage: StorageSettings {
                    db_root: PathBuf::from(".dev-data"),
                    history_window_days: 7,
                    dedup_window_days: 14,
                    max_text_bytes: 4096,
                    gc_batch_size: 200,
                },
                clipboard: nooboard_app::ClipboardSettings {
                    local_capture_enabled: false,
                },
                transfers: TransferSettings {
                    download_dir: PathBuf::from(".dev-data/downloads"),
                },
            },
        }
    }
}
