#![allow(dead_code)]

use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use nooboard_config::APP_CONFIG_VERSION;
use nooboard_core::{
    BootstrapLaunch, BootstrapMode, ClipboardPort, EventId, EventSubscription,
    IncomingTransferDecision, IncomingTransferDisposition, NooboardCore, SessionInfo,
    StateSubscription, UpsertDirectSeedInput, WorkspaceEvent, WorkspaceSnapshot,
};
use nooboard_platform::{ClipboardEvent, ClipboardEventSender};
use tempfile::TempDir;
use tokio::time::{Duration, Instant, sleep, timeout};

pub type TestError = Box<dyn std::error::Error + Send + Sync>;

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
    fn read_text(&self) -> nooboard_core::CoreResult<Option<String>> {
        Ok(self.text.lock().ok().and_then(|value| value.clone()))
    }

    fn write_text(&self, text: &str) -> nooboard_core::CoreResult<()> {
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
    ) -> nooboard_core::CoreResult<JoinHandle<()>> {
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
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind free port");
    let port = listener.local_addr().expect("local addr").port();
    drop(listener);
    port
}

fn write_test_config(
    dir: &TempDir,
    device_id: &str,
    listen_port: u16,
    local_capture_enabled: bool,
) -> Result<PathBuf, TestError> {
    let config_path = dir.path().join("nooboard.toml");
    let noob_id_file = dir.path().join("noob_id");
    let db_root = dir.path().join("data");
    let download_dir = dir.path().join("downloads");
    let raw = format!(
        r#"
[meta]
config_version = {config_version}
profile = "test"

[identity]
noob_id_file = "{noob_id_file}"
device_id = "{device_id}"

[app.clipboard]
recent_event_lookup_limit = 50
local_capture_enabled = {local_capture_enabled}

[storage]
db_root = "{db_root}"
max_text_bytes = 4096
retain_old_versions = 0

[storage.lifecycle]
history_window_days = 7
dedup_window_days = 14
gc_every_inserts = 5
gc_batch_size = 20

[network]
listen_port = {listen_port}

[network.auth]
token = "test-token"

[network.lan]
enabled = false

[network.direct]
approval_timeout_ms = 30000
seeds = []

[network.transfer]
download_dir = "{download_dir}"
max_file_size = 1048576
chunk_size = 4096
active_downloads = 2
decision_timeout_ms = 2000
idle_timeout_ms = 4000

[network.transport]
connect_timeout_ms = 1000
handshake_timeout_ms = 1000
ping_interval_ms = 1000
pong_timeout_ms = 2000
max_packet_size = 65536
"#,
        config_version = APP_CONFIG_VERSION,
        noob_id_file = toml_path(&noob_id_file),
        device_id = device_id,
        local_capture_enabled = local_capture_enabled,
        db_root = toml_path(&db_root),
        listen_port = listen_port,
        download_dir = toml_path(&download_dir),
    );

    std::fs::write(&config_path, raw)?;
    Ok(config_path)
}

pub struct TestCoreEnv {
    pub core: NooboardCore,
    pub backend: Arc<MockClipboardBackend>,
    pub dir: TempDir,
    pub config_path: PathBuf,
    pub listen_port: u16,
}

fn launch_core(
    config_path: &Path,
    backend: Arc<MockClipboardBackend>,
) -> Result<NooboardCore, TestError> {
    let launch = BootstrapLaunch {
        mode: BootstrapMode::ExplicitPath,
        config_path: config_path.to_path_buf(),
    };
    NooboardCore::launch(&launch, backend).map_err(Into::into)
}

pub fn new_core() -> Result<TestCoreEnv, TestError> {
    new_core_with_capture(false)
}

pub fn new_core_with_capture(local_capture_enabled: bool) -> Result<TestCoreEnv, TestError> {
    let dir = TempDir::new()?;
    let listen_port = free_port();
    let config_path = write_test_config(&dir, "test-device", listen_port, local_capture_enabled)?;
    let backend = Arc::new(MockClipboardBackend::default());
    let core = launch_core(&config_path, backend.clone())?;

    Ok(TestCoreEnv {
        core,
        backend,
        dir,
        config_path,
        listen_port,
    })
}

pub fn new_core_pair() -> Result<(TestCoreEnv, TestCoreEnv), TestError> {
    let dir_a = TempDir::new()?;
    let dir_b = TempDir::new()?;
    let listen_port_a = free_port();
    let listen_port_b = free_port();
    let config_path_a = write_test_config(&dir_a, "pair-a-device", listen_port_a, false)?;
    let config_path_b = write_test_config(&dir_b, "pair-b-device", listen_port_b, false)?;
    let backend_a = Arc::new(MockClipboardBackend::default());
    let backend_b = Arc::new(MockClipboardBackend::default());
    let core_a = launch_core(&config_path_a, backend_a.clone())?;
    let core_b = launch_core(&config_path_b, backend_b.clone())?;

    Ok((
        TestCoreEnv {
            core: core_a,
            backend: backend_a,
            dir: dir_a,
            config_path: config_path_a,
            listen_port: listen_port_a,
        },
        TestCoreEnv {
            core: core_b,
            backend: backend_b,
            dir: dir_b,
            config_path: config_path_b,
            listen_port: listen_port_b,
        },
    ))
}

pub fn restart_core(
    config_path: &Path,
    backend: Arc<MockClipboardBackend>,
) -> Result<NooboardCore, TestError> {
    launch_core(config_path, backend)
}

pub async fn wait_for_event<T>(
    subscription: &mut EventSubscription,
    timeout_duration: Duration,
    mut matcher: impl FnMut(WorkspaceEvent) -> Option<T>,
) -> Result<T, TestError> {
    let deadline = Instant::now() + timeout_duration;
    loop {
        let remain = deadline.saturating_duration_since(Instant::now());
        if remain.is_zero() {
            return Err("timed out waiting for workspace event".into());
        }
        let event = timeout(remain, subscription.recv()).await??;
        if let Some(value) = matcher(event) {
            return Ok(value);
        }
    }
}

pub async fn wait_for_state_update(
    subscription: &mut StateSubscription,
    timeout_duration: Duration,
    predicate: impl Fn(&WorkspaceSnapshot) -> bool,
) -> Result<WorkspaceSnapshot, TestError> {
    if predicate(subscription.latest()) {
        return Ok(subscription.latest().clone());
    }

    let deadline = Instant::now() + timeout_duration;
    loop {
        let remain = deadline.saturating_duration_since(Instant::now());
        if remain.is_zero() {
            return Err("timed out waiting for state update".into());
        }
        let snapshot = timeout(remain, subscription.recv()).await??;
        if predicate(&snapshot) {
            return Ok(snapshot);
        }
    }
}

pub async fn wait_for_snapshot(
    core: &NooboardCore,
    timeout_duration: Duration,
    predicate: impl Fn(&WorkspaceSnapshot) -> bool,
) -> Result<WorkspaceSnapshot, TestError> {
    let deadline = Instant::now() + timeout_duration;
    loop {
        let snapshot = core.snapshot().await?;
        if predicate(&snapshot) {
            return Ok(snapshot);
        }
        if Instant::now() >= deadline {
            return Err("timed out waiting for snapshot predicate".into());
        }
        sleep(Duration::from_millis(25)).await;
    }
}

pub async fn wait_for_sessions(
    core: &NooboardCore,
    expected: usize,
) -> Result<Vec<SessionInfo>, TestError> {
    timeout(Duration::from_secs(10), async {
        loop {
            let sessions = core.list_sessions().await?;
            if sessions.len() == expected {
                return Ok::<_, TestError>(sessions);
            }
            sleep(Duration::from_millis(25)).await;
        }
    })
    .await?
}

pub async fn connect_core_pair(
    core_a: &NooboardCore,
    core_b: &NooboardCore,
    port_b: u16,
) -> Result<(), TestError> {
    core_a.start_network().await?;
    core_b.start_network().await?;

    let seed_id = core_a
        .upsert_direct_seed(UpsertDirectSeedInput {
            id: None,
            label: "peer-b".to_string(),
            host: "127.0.0.1".to_string(),
            port: port_b,
            enabled: true,
        })
        .await?;
    let _ = core_a.connect_direct_seed(seed_id).await?;

    let pending = timeout(Duration::from_secs(10), async {
        loop {
            let pending = core_b.list_pending_direct_requests().await?;
            if let Some(request) = pending.into_iter().next() {
                return Ok::<_, TestError>(request);
            }
            sleep(Duration::from_millis(25)).await;
        }
    })
    .await??;
    core_b.approve_direct_request(pending.id).await?;

    let _ = wait_for_sessions(core_a, 1).await?;
    let _ = wait_for_sessions(core_b, 1).await?;
    Ok(())
}

pub fn committed_clipboard_event(
    event: WorkspaceEvent,
) -> Option<(EventId, nooboard_core::ClipboardRecordSource)> {
    match event {
        WorkspaceEvent::ClipboardCommitted { event_id, source } => Some((event_id, source)),
        _ => None,
    }
}

pub fn transfer_completed_event(
    event: WorkspaceEvent,
) -> Option<(
    nooboard_core::TransferTicket,
    nooboard_core::TransferOutcome,
)> {
    match event {
        WorkspaceEvent::TransferCompleted { ticket, outcome } => Some((ticket, outcome)),
        _ => None,
    }
}

pub fn incoming_transfer_ticket(event: WorkspaceEvent) -> Option<nooboard_core::TransferTicket> {
    match event {
        WorkspaceEvent::IncomingTransferOffered { ticket } => Some(ticket),
        _ => None,
    }
}

pub async fn accept_incoming_transfer(
    core: &NooboardCore,
    ticket: nooboard_core::TransferTicket,
) -> Result<(), TestError> {
    core.decide_incoming_transfer(IncomingTransferDecision {
        ticket,
        decision: IncomingTransferDisposition::Accept,
    })
    .await?;
    Ok(())
}
