use std::net::{SocketAddr, TcpListener};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use nooboard_app::{
    AppError, AppEvent, ClipboardPort, ClipboardRecordSource, DesktopAppService,
    DesktopAppServiceImpl, EventId, EventSubscription, ListClipboardHistoryRequest,
    NetworkSettingsPatch, SettingsPatch, SubmitTextRequest, SyncDesiredState,
};
use tempfile::TempDir;
use tokio::time::{Duration, timeout};

#[derive(Default)]
struct MockClipboardBackend {
    text: Mutex<Option<String>>,
    writes: Mutex<Vec<String>>,
}

impl MockClipboardBackend {
    fn last_written(&self) -> Option<String> {
        self.writes
            .lock()
            .ok()
            .and_then(|writes| writes.last().cloned())
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

fn write_test_config(dir: &TempDir) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let config_path = dir.path().join("app.toml");
    let noob_id_file = dir.path().join("noob_id");
    let db_root = dir.path().join("db");
    let download_dir = dir.path().join("downloads");
    let listen_addr: SocketAddr = format!("127.0.0.1:{}", free_port()).parse()?;

    let raw = format!(
        r#"
[meta]
config_version = 2
profile = "test"

[identity]
noob_id_file = "{noob_id_file}"
device_id = "test-device"

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
mdns_enabled = true
listen_addr = "{listen_addr}"
manual_peers = []

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
        db_root = toml_path(&db_root),
        download_dir = toml_path(&download_dir),
        listen_addr = listen_addr,
    );

    std::fs::write(&config_path, raw)?;
    Ok(config_path)
}

fn new_service() -> Result<
    (
        DesktopAppServiceImpl,
        Arc<MockClipboardBackend>,
        TempDir,
        PathBuf,
    ),
    Box<dyn std::error::Error>,
> {
    let dir = TempDir::new()?;
    let config_path = write_test_config(&dir)?;
    let backend = Arc::new(MockClipboardBackend::default());
    let service = DesktopAppServiceImpl::new_with_clipboard(&config_path, backend.clone())?;
    Ok((service, backend, dir, config_path))
}

async fn recv_clipboard_committed(
    subscription: &mut EventSubscription,
) -> Result<(EventId, ClipboardRecordSource), Box<dyn std::error::Error>> {
    let event = timeout(Duration::from_secs(2), subscription.recv()).await??;
    match event {
        AppEvent::ClipboardCommitted { event_id, source } => Ok((event_id, source)),
        other => Err(format!("unexpected event: {other:?}").into()),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn subscriptions_are_app_lifetime_before_sync_starts()
-> Result<(), Box<dyn std::error::Error>> {
    let (service, _backend, _dir, _config_path) = new_service()?;

    let mut state_subscription = service.subscribe_state().await?;
    let mut event_subscription = service.subscribe_events().await?;
    assert_eq!(
        state_subscription.latest().sync.desired,
        SyncDesiredState::Stopped
    );
    assert_eq!(
        state_subscription
            .latest()
            .clipboard
            .latest_committed_event_id,
        None
    );

    service
        .patch_settings(SettingsPatch::Network(
            NetworkSettingsPatch::SetMdnsEnabled(false),
        ))
        .await?;

    let next_state = timeout(Duration::from_secs(2), state_subscription.recv()).await??;
    assert!(!next_state.settings.network.mdns_enabled);
    assert_eq!(next_state.sync.desired, SyncDesiredState::Stopped);

    let submitted_event_id = service
        .submit_text(SubmitTextRequest {
            content: "alpha".to_string(),
        })
        .await?;
    let (observed_event_id, source) = recv_clipboard_committed(&mut event_subscription).await?;
    assert_eq!(observed_event_id, submitted_event_id);
    assert_eq!(source, ClipboardRecordSource::UserSubmit);

    let committed_state = timeout(Duration::from_secs(2), state_subscription.recv()).await??;
    assert_eq!(
        committed_state.clipboard.latest_committed_event_id,
        Some(submitted_event_id)
    );

    service.shutdown().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn clipboard_committed_event_only_follows_successful_commit()
-> Result<(), Box<dyn std::error::Error>> {
    let (service, _backend, _dir, _config_path) = new_service()?;
    let mut event_subscription = service.subscribe_events().await?;

    let event_id = service
        .submit_text(SubmitTextRequest {
            content: "short".to_string(),
        })
        .await?;
    let (committed_event_id, _) = recv_clipboard_committed(&mut event_subscription).await?;
    assert_eq!(committed_event_id, event_id);

    let record = service.get_clipboard_record(event_id).await?;
    assert_eq!(record.content, "short");

    let too_large = "x".repeat(65);
    let error = service
        .submit_text(SubmitTextRequest { content: too_large })
        .await
        .expect_err("oversized content must fail");
    assert!(matches!(error, AppError::TextTooLarge { .. }));

    let maybe_event = timeout(Duration::from_millis(200), event_subscription.recv()).await;
    assert!(
        maybe_event.is_err(),
        "failed commit must not emit ClipboardCommitted"
    );

    service.shutdown().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn adopt_clipboard_record_does_not_create_new_record()
-> Result<(), Box<dyn std::error::Error>> {
    let (service, backend, _dir, _config_path) = new_service()?;

    let event_id = service
        .submit_text(SubmitTextRequest {
            content: "adopt-me".to_string(),
        })
        .await?;

    let before = service
        .list_clipboard_history(ListClipboardHistoryRequest {
            limit: 10,
            cursor: None,
        })
        .await?;
    assert_eq!(before.records.len(), 1);

    service.adopt_clipboard_record(event_id).await?;
    assert_eq!(backend.last_written().as_deref(), Some("adopt-me"));

    let after = service
        .list_clipboard_history(ListClipboardHistoryRequest {
            limit: 10,
            cursor: None,
        })
        .await?;
    assert_eq!(after.records.len(), 1);
    assert_eq!(after.records[0].event_id, event_id);

    service.shutdown().await?;
    Ok(())
}
