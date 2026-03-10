#![allow(dead_code)]

use std::net::{SocketAddr, TcpListener};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use nooboard_app::{
    AppEvent, AppState, ClipboardPort, ClipboardRecordSource, DesktopAppService,
    DesktopAppServiceImpl, EventId, EventSubscription, NoobId, StateSubscription, SyncActualStatus,
    SyncDesiredState,
};
use nooboard_platform::{ClipboardEvent, ClipboardEventSender};
use tempfile::TempDir;
use tokio::time::{Duration, Instant, sleep, timeout};

pub type TestError = Box<dyn std::error::Error>;

#[derive(Default)]
pub struct MockClipboardBackend {
    text: Mutex<Option<String>>,
    writes: Mutex<Vec<String>>,
    watchers: Mutex<Vec<ClipboardEventSender>>,
}

impl MockClipboardBackend {
    pub fn last_written(&self) -> Option<String> {
        self.writes
            .lock()
            .ok()
            .and_then(|writes| writes.last().cloned())
    }

    pub fn emit_watch_text(&self, text: &str) {
        let event = ClipboardEvent::new(text.to_string());
        if let Ok(watchers) = self.watchers.lock() {
            for sender in watchers.iter() {
                let _ = sender.try_send(event.clone());
            }
        }
    }
}

impl ClipboardPort for MockClipboardBackend {
    fn read_text(&self) -> nooboard_app::AppResult<Option<String>> {
        Ok(self.text.lock().ok().and_then(|value| value.clone()))
    }

    fn write_text(&self, text: &str) -> nooboard_app::AppResult<()> {
        if let Ok(mut value) = self.text.lock() {
            *value = Some(text.to_string());
        }
        if let Ok(mut writes) = self.writes.lock() {
            writes.push(text.to_string());
        }
        Ok(())
    }

    fn watch_changes(
        &self,
        sender: ClipboardEventSender,
        shutdown: Arc<AtomicBool>,
        _interval: Duration,
    ) -> nooboard_app::AppResult<JoinHandle<()>> {
        if let Ok(mut watchers) = self.watchers.lock() {
            watchers.push(sender);
        }
        Ok(std::thread::spawn(move || {
            while !shutdown.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(10));
            }
        }))
    }
}

fn toml_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "\\\\")
}

fn free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("must bind ephemeral port");
    let port = listener
        .local_addr()
        .expect("must resolve local addr")
        .port();
    drop(listener);
    port
}

fn write_test_config(
    dir: &TempDir,
    device_id: &str,
    listen_addr: SocketAddr,
    manual_peers: &[SocketAddr],
    mdns_enabled: bool,
) -> Result<PathBuf, TestError> {
    let config_path = dir.path().join("app.toml");
    let noob_id_file = dir.path().join("noob_id");
    let db_root = dir.path().join("db");
    let download_dir = dir.path().join("downloads");
    let manual_peers = manual_peers
        .iter()
        .map(|addr| format!("\"{addr}\""))
        .collect::<Vec<_>>()
        .join(", ");

    let raw = format!(
        r#"
[meta]
config_version = 2
profile = "test"

[identity]
noob_id_file = "{noob_id_file}"
device_id = "{device_id}"

[app.clipboard]
recent_event_lookup_limit = 50
local_capture_enabled = false

[storage]
db_root = "{db_root}"
max_text_bytes = 64
retain_old_versions = 0

[storage.lifecycle]
history_window_days = 7
dedup_window_days = 14
gc_every_inserts = 5
gc_batch_size = 20

[sync.network]
enabled = true
mdns_enabled = {mdns_enabled}
listen_addr = "{listen_addr}"
manual_peers = [{manual_peers}]

[sync.auth]
token = "test-sync-token"

[sync.file]
download_dir = "{download_dir}"
max_file_size = 1048576
chunk_size = 4096
active_downloads = 2
decision_timeout_ms = 2000
idle_timeout_ms = 2000

[sync.transport]
connect_timeout_ms = 1000
handshake_timeout_ms = 1000
ping_interval_ms = 1000
pong_timeout_ms = 2000
max_packet_size = 65536
"#,
        noob_id_file = toml_path(&noob_id_file),
        device_id = device_id,
        db_root = toml_path(&db_root),
        download_dir = toml_path(&download_dir),
        listen_addr = listen_addr,
        mdns_enabled = mdns_enabled,
        manual_peers = manual_peers,
    );

    std::fs::write(&config_path, raw)?;
    Ok(config_path)
}

pub struct TestServiceEnv {
    pub service: DesktopAppServiceImpl,
    pub backend: Arc<MockClipboardBackend>,
    pub dir: TempDir,
    pub config_path: PathBuf,
    pub listen_addr: SocketAddr,
}

fn new_service_with_network(
    device_id: &str,
    listen_addr: SocketAddr,
    manual_peers: &[SocketAddr],
    mdns_enabled: bool,
) -> Result<TestServiceEnv, TestError> {
    let dir = TempDir::new()?;
    let config_path = write_test_config(&dir, device_id, listen_addr, manual_peers, mdns_enabled)?;
    let backend = Arc::new(MockClipboardBackend::default());
    let service = DesktopAppServiceImpl::new_with_clipboard(&config_path, backend.clone())?;
    Ok(TestServiceEnv {
        service,
        backend,
        dir,
        config_path,
        listen_addr,
    })
}

pub fn new_service() -> Result<TestServiceEnv, TestError> {
    let listen_addr: SocketAddr = format!("127.0.0.1:{}", free_port()).parse()?;
    new_service_with_network("test-device", listen_addr, &[], true)
}

pub fn new_service_pair() -> Result<(TestServiceEnv, TestServiceEnv), TestError> {
    let listen_addr_a: SocketAddr = format!("127.0.0.1:{}", free_port()).parse()?;
    let listen_addr_b: SocketAddr = format!("127.0.0.1:{}", free_port()).parse()?;
    let service_a =
        new_service_with_network("pair-a-device", listen_addr_a, &[listen_addr_b], false)?;
    let service_b =
        new_service_with_network("pair-b-device", listen_addr_b, &[listen_addr_a], false)?;
    Ok((service_a, service_b))
}

pub fn new_service_fanout() -> Result<(TestServiceEnv, TestServiceEnv, TestServiceEnv), TestError> {
    let listen_addr_a: SocketAddr = format!("127.0.0.1:{}", free_port()).parse()?;
    let listen_addr_b: SocketAddr = format!("127.0.0.1:{}", free_port()).parse()?;
    let listen_addr_c: SocketAddr = format!("127.0.0.1:{}", free_port()).parse()?;
    let service_a = new_service_with_network(
        "fanout-a-device",
        listen_addr_a,
        &[listen_addr_b, listen_addr_c],
        false,
    )?;
    let service_b =
        new_service_with_network("fanout-b-device", listen_addr_b, &[listen_addr_a], false)?;
    let service_c =
        new_service_with_network("fanout-c-device", listen_addr_c, &[listen_addr_a], false)?;
    Ok((service_a, service_b, service_c))
}

pub fn restart_service(
    config_path: &Path,
    backend: Arc<MockClipboardBackend>,
) -> Result<DesktopAppServiceImpl, TestError> {
    DesktopAppServiceImpl::new_with_clipboard(config_path, backend).map_err(Into::into)
}

pub async fn recv_clipboard_committed(
    subscription: &mut EventSubscription,
) -> Result<(EventId, ClipboardRecordSource), TestError> {
    let event = timeout(Duration::from_secs(2), subscription.recv()).await??;
    match event {
        AppEvent::ClipboardCommitted { event_id, source } => Ok((event_id, source)),
        other => Err(format!("unexpected event: {other:?}").into()),
    }
}

pub async fn wait_for_service_state(
    service: &DesktopAppServiceImpl,
    timeout_duration: Duration,
    predicate: impl Fn(&AppState) -> bool,
) -> Result<AppState, TestError> {
    let deadline = Instant::now() + timeout_duration;
    loop {
        let state = service.get_state().await?;
        if predicate(&state) {
            return Ok(state);
        }
        if Instant::now() >= deadline {
            return Err("timed out waiting for service state".into());
        }
        sleep(Duration::from_millis(50)).await;
    }
}

pub async fn wait_for_state_update(
    subscription: &mut StateSubscription,
    timeout_duration: Duration,
    predicate: impl Fn(&AppState) -> bool,
) -> Result<AppState, TestError> {
    if predicate(subscription.latest()) {
        return Ok(subscription.latest().clone());
    }

    let deadline = Instant::now() + timeout_duration;
    loop {
        let remain = deadline.saturating_duration_since(Instant::now());
        if remain.is_zero() {
            return Err("timed out waiting for subscribed state update".into());
        }
        let state = timeout(remain, subscription.recv()).await??;
        if predicate(&state) {
            return Ok(state);
        }
    }
}

pub async fn wait_for_event<T>(
    subscription: &mut EventSubscription,
    timeout_duration: Duration,
    mut matcher: impl FnMut(AppEvent) -> Option<T>,
) -> Result<T, TestError> {
    let deadline = Instant::now() + timeout_duration;
    loop {
        let remain = deadline.saturating_duration_since(Instant::now());
        if remain.is_zero() {
            return Err("timed out waiting for event".into());
        }
        let event = timeout(remain, subscription.recv()).await??;
        if let Some(value) = matcher(event) {
            return Ok(value);
        }
    }
}

pub async fn connect_service_pair(
    service_a: &DesktopAppServiceImpl,
    service_b: &DesktopAppServiceImpl,
) -> Result<(NoobId, NoobId), TestError> {
    let noob_id_a = service_a.get_state().await?.identity.noob_id;
    let noob_id_b = service_b.get_state().await?.identity.noob_id;

    service_a
        .set_sync_desired_state(SyncDesiredState::Running)
        .await?;
    service_b
        .set_sync_desired_state(SyncDesiredState::Running)
        .await?;

    wait_for_service_state(service_a, Duration::from_secs(10), |state| {
        state.sync.actual == SyncActualStatus::Running
            && state
                .peers
                .connected
                .iter()
                .any(|peer| peer.noob_id == noob_id_b)
    })
    .await?;
    wait_for_service_state(service_b, Duration::from_secs(10), |state| {
        state.sync.actual == SyncActualStatus::Running
            && state
                .peers
                .connected
                .iter()
                .any(|peer| peer.noob_id == noob_id_a)
    })
    .await?;

    Ok((noob_id_a, noob_id_b))
}

pub async fn connect_service_fanout(
    service_a: &DesktopAppServiceImpl,
    service_b: &DesktopAppServiceImpl,
    service_c: &DesktopAppServiceImpl,
) -> Result<(NoobId, NoobId, NoobId), TestError> {
    let noob_id_a = service_a.get_state().await?.identity.noob_id;
    let noob_id_b = service_b.get_state().await?.identity.noob_id;
    let noob_id_c = service_c.get_state().await?.identity.noob_id;

    service_a
        .set_sync_desired_state(SyncDesiredState::Running)
        .await?;
    service_b
        .set_sync_desired_state(SyncDesiredState::Running)
        .await?;
    service_c
        .set_sync_desired_state(SyncDesiredState::Running)
        .await?;

    wait_for_service_state(service_a, Duration::from_secs(10), |state| {
        state.sync.actual == SyncActualStatus::Running
            && state
                .peers
                .connected
                .iter()
                .any(|peer| peer.noob_id == noob_id_b)
            && state
                .peers
                .connected
                .iter()
                .any(|peer| peer.noob_id == noob_id_c)
    })
    .await?;
    wait_for_service_state(service_b, Duration::from_secs(10), |state| {
        state.sync.actual == SyncActualStatus::Running
            && state
                .peers
                .connected
                .iter()
                .any(|peer| peer.noob_id == noob_id_a)
    })
    .await?;
    wait_for_service_state(service_c, Duration::from_secs(10), |state| {
        state.sync.actual == SyncActualStatus::Running
            && state
                .peers
                .connected
                .iter()
                .any(|peer| peer.noob_id == noob_id_a)
    })
    .await?;

    Ok((noob_id_a, noob_id_b, noob_id_c))
}
