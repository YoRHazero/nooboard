use std::net::{SocketAddr, TcpListener};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use nooboard_app::{
    AppConfig, AppError, AppEvent, AppResult, AppService, AppServiceImpl, ClipboardPort, EventId,
    EventSubscriptionItem, FileDecisionRequest, ListHistoryRequest, LocalClipboardChangeRequest,
    NetworkPatch, NodeId, RebroadcastHistoryRequest, RemoteTextRequest, SendFileRequest,
    SubscriptionCloseReason, SubscriptionLifecycle, SyncEvent, Targets, TransferState,
};
use tempfile::TempDir;
use tokio::time::{Duration, Instant, timeout};

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
    fn read_text(&self) -> AppResult<Option<String>> {
        Ok(self.text.lock().ok().and_then(|value| value.clone()))
    }

    fn write_text(&self, text: &str) -> AppResult<()> {
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

fn write_test_config(
    dir: &TempDir,
    recent_limit: usize,
    listen_addr: SocketAddr,
    manual_peers: &[SocketAddr],
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let config_path = dir.path().join("app.toml");
    let noob_id_file = dir.path().join("node_id");
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
device_id = "test-device"

[app.clipboard]
recent_event_lookup_limit = {recent_limit}

[storage]
db_root = "{db_root}"
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
        db_root = toml_path(&db_root),
        download_dir = toml_path(&download_dir),
        recent_limit = recent_limit,
        listen_addr = listen_addr,
        manual_peers = manual_peers,
    );

    std::fs::write(&config_path, raw)?;
    Ok(config_path)
}

fn new_service(
    recent_limit: usize,
) -> Result<(AppServiceImpl, Arc<MockClipboardBackend>, TempDir, PathBuf), Box<dyn std::error::Error>>
{
    let dir = TempDir::new()?;
    let config_path = write_test_config(
        &dir,
        recent_limit,
        "127.0.0.1:0".parse().expect("valid loopback addr"),
        &[],
    )?;
    let backend = Arc::new(MockClipboardBackend::default());
    let service = AppServiceImpl::new(&config_path, backend.clone())?;
    Ok((service, backend, dir, config_path))
}

fn new_service_with_network(
    recent_limit: usize,
    listen_addr: SocketAddr,
    manual_peers: Vec<SocketAddr>,
) -> Result<(AppServiceImpl, Arc<MockClipboardBackend>, TempDir, PathBuf), Box<dyn std::error::Error>>
{
    let dir = TempDir::new()?;
    let config_path = write_test_config(&dir, recent_limit, listen_addr, &manual_peers)?;
    let backend = Arc::new(MockClipboardBackend::default());
    let service = AppServiceImpl::new(&config_path, backend.clone())?;
    Ok((service, backend, dir, config_path))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn clipboard_flow_covers_a1_a2_a3_a4() -> Result<(), Box<dyn std::error::Error>> {
    let (service, backend, _dir, _config_path) = new_service(50)?;
    service.start_engine().await?;

    let event_id = service
        .apply_local_clipboard_change(LocalClipboardChangeRequest {
            text: "alpha".to_string(),
            targets: Targets::all(),
        })
        .await?
        .event_id;

    let history = service
        .list_history(ListHistoryRequest {
            limit: 10,
            cursor: None,
        })
        .await?;
    assert_eq!(history.records.len(), 1);
    assert_eq!(history.records[0].event_id, event_id);
    assert_eq!(history.records[0].content, "alpha");

    service.apply_history_entry_to_clipboard(event_id).await?;
    assert_eq!(backend.last_written().as_deref(), Some("alpha"));

    service
        .rebroadcast_history_entry(RebroadcastHistoryRequest {
            event_id,
            targets: Targets::nodes(vec![NodeId::new("peer-node-x")]),
        })
        .await?;

    service.stop_engine().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn local_clipboard_change_succeeds_when_network_disabled()
-> Result<(), Box<dyn std::error::Error>> {
    let (service, _backend, _dir, _config_path) = new_service(50)?;
    service.start_engine().await?;
    service
        .apply_network_patch(NetworkPatch::SetNetworkEnabled(false))
        .await?;

    let result = service
        .apply_local_clipboard_change(LocalClipboardChangeRequest {
            text: "offline-local".to_string(),
            targets: Targets::all(),
        })
        .await?;
    assert!(!result.broadcast_attempted);

    let history = service
        .list_history(ListHistoryRequest {
            limit: 10,
            cursor: None,
        })
        .await?;
    assert_eq!(history.records.len(), 1);
    assert_eq!(history.records[0].event_id, result.event_id);
    assert_eq!(history.records[0].content, "offline-local");

    service.stop_engine().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn recent_window_not_found_returns_business_error() -> Result<(), Box<dyn std::error::Error>>
{
    let (service, _backend, _dir, _config_path) = new_service(1)?;

    let old_event_id = EventId::from(uuid::Uuid::now_v7());
    let new_event_id = EventId::from(uuid::Uuid::now_v7());

    service
        .store_remote_text(RemoteTextRequest {
            event_id: old_event_id,
            content: "old".to_string(),
            device_id: "remote-a".to_string(),
        })
        .await?;
    service
        .store_remote_text(RemoteTextRequest {
            event_id: new_event_id,
            content: "new".to_string(),
            device_id: "remote-b".to_string(),
        })
        .await?;

    let apply_result = service.apply_history_entry_to_clipboard(old_event_id).await;
    assert!(matches!(
        apply_result,
        Err(AppError::NotFoundInRecentWindow { .. })
    ));

    let rebroadcast_result = service
        .rebroadcast_history_entry(RebroadcastHistoryRequest {
            event_id: old_event_id,
            targets: Targets::nodes(vec![NodeId::new("peer-node-y")]),
        })
        .await;
    assert!(matches!(
        rebroadcast_result,
        Err(AppError::NotFoundInRecentWindow { .. })
    ));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_storage_and_clipboard_apis_are_decoupled() -> Result<(), Box<dyn std::error::Error>>
{
    let (service, backend, _dir, _config_path) = new_service(50)?;

    service
        .store_remote_text(RemoteTextRequest {
            event_id: EventId::new(),
            content: "from-remote-storage".to_string(),
            device_id: "remote-node".to_string(),
        })
        .await?;

    let before = service
        .list_history(ListHistoryRequest {
            limit: 10,
            cursor: None,
        })
        .await?;
    assert_eq!(before.records.len(), 1);

    service
        .write_remote_text_to_clipboard(RemoteTextRequest {
            event_id: EventId::new(),
            content: "from-remote-clipboard".to_string(),
            device_id: "remote-node".to_string(),
        })
        .await?;

    let after = service
        .list_history(ListHistoryRequest {
            limit: 10,
            cursor: None,
        })
        .await?;
    assert_eq!(after.records.len(), 1);
    assert_eq!(
        backend.last_written().as_deref(),
        Some("from-remote-clipboard")
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_api_supports_normal_and_error_paths() -> Result<(), Box<dyn std::error::Error>> {
    let (service, _backend, dir, _config_path) = new_service(50)?;
    let file_path = dir.path().join("sample.txt");
    std::fs::write(&file_path, "file-body")?;

    let result_before_start = service
        .send_file(SendFileRequest {
            path: file_path.clone(),
            targets: Targets::all(),
        })
        .await;
    assert!(matches!(
        result_before_start,
        Err(AppError::EngineNotRunning)
    ));

    service.start_engine().await?;
    service
        .send_file(SendFileRequest {
            path: file_path.clone(),
            targets: Targets::all(),
        })
        .await?;
    service
        .respond_file_decision(FileDecisionRequest {
            peer_node_id: NodeId::new("ghost-peer"),
            transfer_id: 1,
            accept: false,
            reason: Some("reject".to_string()),
        })
        .await?;

    let mut events = service.subscribe_events().await?;
    assert!(matches!(
        events.try_recv(),
        Ok(EventSubscriptionItem::Lifecycle(
            SubscriptionLifecycle::Opened { .. }
        ))
    ));
    assert!(events.try_recv().is_err());

    service.stop_engine().await?;

    let result_after_stop = service
        .send_file(SendFileRequest {
            path: file_path,
            targets: Targets::all(),
        })
        .await;
    assert!(matches!(result_after_stop, Err(AppError::EngineNotRunning)));
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn broadcast_config_transaction_covers_normal_and_error_paths()
-> Result<(), Box<dyn std::error::Error>> {
    let (service, _backend, _dir, config_path) = new_service(50)?;
    let service = Arc::new(service);

    service.start_engine().await?;
    let result = service
        .apply_network_patch(NetworkPatch::SetMdnsEnabled(false))
        .await?;
    assert!(!result.mdns_enabled);

    let peer_a: SocketAddr = "127.0.0.1:18001".parse()?;
    let peer_b: SocketAddr = "127.0.0.1:18002".parse()?;

    let service_a = Arc::clone(&service);
    let add_a = tokio::spawn(async move {
        service_a
            .apply_network_patch(NetworkPatch::AddManualPeer(peer_a))
            .await
    });
    let service_b = Arc::clone(&service);
    let add_b = tokio::spawn(async move {
        service_b
            .apply_network_patch(NetworkPatch::AddManualPeer(peer_b))
            .await
    });

    add_a.await??;
    add_b.await??;

    let duplicate_add = service
        .apply_network_patch(NetworkPatch::AddManualPeer(peer_a))
        .await;
    assert!(matches!(
        duplicate_add,
        Err(AppError::ManualPeerExists { .. })
    ));

    service
        .apply_network_patch(NetworkPatch::RemoveManualPeer(peer_a))
        .await?;
    let missing_remove = service
        .apply_network_patch(NetworkPatch::RemoveManualPeer(peer_a))
        .await;
    assert!(matches!(
        missing_remove,
        Err(AppError::ManualPeerNotFound { .. })
    ));

    let persisted = AppConfig::load(&config_path)?;
    assert!(!persisted.sync.network.mdns_enabled);
    assert_eq!(persisted.sync.network.manual_peers, vec![peer_b]);

    service.stop_engine().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn transfer_events_do_not_break_sync_event_bridge() -> Result<(), Box<dyn std::error::Error>>
{
    let port_a = free_port();
    let port_b = free_port();
    let listen_a: SocketAddr = format!("127.0.0.1:{port_a}").parse()?;
    let listen_b: SocketAddr = format!("127.0.0.1:{port_b}").parse()?;

    let (service_a, _backend_a, dir_a, _config_a) =
        new_service_with_network(50, listen_a, vec![listen_b])?;
    let (service_b, _backend_b, _dir_b, _config_b) =
        new_service_with_network(50, listen_b, vec![listen_a])?;

    service_a.start_engine().await?;
    service_b.start_engine().await?;

    let mut receiver_b = service_b.subscribe_events().await?;

    let connect_deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < connect_deadline {
        if !service_a.connected_peers().await?.is_empty()
            && !service_b.connected_peers().await?.is_empty()
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(
        !service_a.connected_peers().await?.is_empty(),
        "service A must connect to service B before file transfer"
    );
    assert!(
        !service_b.connected_peers().await?.is_empty(),
        "service B must connect to service A before file transfer"
    );

    let file_path = dir_a.path().join("bridge-check.txt");
    std::fs::write(&file_path, "bridge-check-body")?;
    service_a
        .send_file(SendFileRequest {
            path: file_path,
            targets: Targets::all(),
        })
        .await?;

    let transfer_deadline = Instant::now() + Duration::from_secs(15);
    let mut transfer_finished = false;
    while Instant::now() < transfer_deadline {
        let remain = transfer_deadline.saturating_duration_since(Instant::now());
        if remain.is_zero() {
            break;
        }

        let item = timeout(remain, receiver_b.recv()).await??;
        let EventSubscriptionItem::Event { event, .. } = item else {
            continue;
        };
        match event {
            AppEvent::Sync(SyncEvent::FileDecisionRequired {
                peer_node_id,
                transfer_id,
                ..
            }) => {
                service_b
                    .respond_file_decision(FileDecisionRequest {
                        peer_node_id,
                        transfer_id,
                        accept: true,
                        reason: None,
                    })
                    .await?;
            }
            AppEvent::Transfer(update) => {
                if matches!(update.state, TransferState::Finished { .. }) {
                    transfer_finished = true;
                    break;
                }
            }
            AppEvent::Sync(_) => {}
        }
    }
    assert!(transfer_finished, "must observe transfer finished event");

    let marker = "after-transfer";
    service_a
        .apply_local_clipboard_change(LocalClipboardChangeRequest {
            text: marker.to_string(),
            targets: Targets::all(),
        })
        .await?;

    let text_deadline = Instant::now() + Duration::from_secs(10);
    let mut saw_text_received = false;
    while Instant::now() < text_deadline {
        let remain = text_deadline.saturating_duration_since(Instant::now());
        if remain.is_zero() {
            break;
        }

        let item = timeout(remain, receiver_b.recv()).await??;
        let EventSubscriptionItem::Event { event, .. } = item else {
            continue;
        };
        if let AppEvent::Sync(SyncEvent::TextReceived { content, .. }) = event
            && content == marker
        {
            saw_text_received = true;
            break;
        }
    }
    assert!(
        saw_text_received,
        "text events must still flow after transfer updates"
    );

    service_a.stop_engine().await?;
    service_b.stop_engine().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn repeated_subscribe_uses_shared_hub() -> Result<(), Box<dyn std::error::Error>> {
    let (service, _backend, _dir, _config_path) = new_service(50)?;
    service.start_engine().await?;

    for _ in 0..100 {
        let receiver = service.subscribe_events().await?;
        drop(receiver);
    }

    service
        .apply_local_clipboard_change(LocalClipboardChangeRequest {
            text: "ping".to_string(),
            targets: Targets::all(),
        })
        .await?;

    let mut receiver = service.subscribe_events().await?;
    assert!(matches!(
        receiver.try_recv(),
        Ok(EventSubscriptionItem::Lifecycle(
            SubscriptionLifecycle::Opened { .. }
        ))
    ));
    assert!(receiver.try_recv().is_err());

    service.stop_engine().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stop_emits_terminal_closed_lifecycle() -> Result<(), Box<dyn std::error::Error>> {
    let (service, _backend, _dir, _config_path) = new_service(50)?;
    service.start_engine().await?;

    let mut subscription = service.subscribe_events().await?;
    let session_id = subscription.session_id();
    assert_eq!(
        subscription.recv().await?,
        EventSubscriptionItem::Lifecycle(SubscriptionLifecycle::Opened { session_id })
    );

    service.stop_engine().await?;

    assert_eq!(
        timeout(Duration::from_secs(2), subscription.recv()).await??,
        EventSubscriptionItem::Lifecycle(SubscriptionLifecycle::Closed {
            session_id,
            reason: SubscriptionCloseReason::EngineStopped,
        })
    );
    assert!(matches!(
        timeout(Duration::from_secs(2), subscription.recv()).await,
        Ok(Err(tokio::sync::broadcast::error::RecvError::Closed))
    ));
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn restart_rebinds_old_stream_and_keeps_new_stream_usable()
-> Result<(), Box<dyn std::error::Error>> {
    let port_a = free_port();
    let port_b = free_port();
    let listen_a: SocketAddr = format!("127.0.0.1:{port_a}").parse()?;
    let listen_b: SocketAddr = format!("127.0.0.1:{port_b}").parse()?;

    let (service_a, _backend_a, _dir_a, _config_a) =
        new_service_with_network(50, listen_a, vec![listen_b])?;
    let (service_b, _backend_b, _dir_b, _config_b) =
        new_service_with_network(50, listen_b, vec![listen_a])?;

    service_a.start_engine().await?;
    service_b.start_engine().await?;

    let mut old_stream = service_b.subscribe_events().await?;
    let old_session_id = old_stream.session_id();
    assert_eq!(
        old_stream.recv().await?,
        EventSubscriptionItem::Lifecycle(SubscriptionLifecycle::Opened {
            session_id: old_session_id,
        })
    );

    let connect_deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < connect_deadline {
        if !service_a.connected_peers().await?.is_empty()
            && !service_b.connected_peers().await?.is_empty()
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(
        !service_a.connected_peers().await?.is_empty(),
        "service A must connect to service B before restart"
    );
    assert!(
        !service_b.connected_peers().await?.is_empty(),
        "service B must connect to service A before restart"
    );

    service_b.restart_engine().await?;

    let mut saw_rebinding = false;
    let mut saw_closed = false;
    let old_stream_deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < old_stream_deadline {
        let remain = old_stream_deadline.saturating_duration_since(Instant::now());
        if remain.is_zero() {
            break;
        }

        match timeout(remain, old_stream.recv()).await?? {
            EventSubscriptionItem::Lifecycle(SubscriptionLifecycle::Rebinding {
                from_session_id,
                to_session_id,
            }) => {
                assert_eq!(from_session_id, old_session_id);
                assert!(to_session_id > old_session_id);
                saw_rebinding = true;
            }
            EventSubscriptionItem::Lifecycle(SubscriptionLifecycle::Closed {
                session_id,
                reason: SubscriptionCloseReason::Rebinding { next_session_id },
            }) => {
                assert_eq!(session_id, old_session_id);
                assert!(next_session_id > old_session_id);
                saw_closed = true;
                break;
            }
            EventSubscriptionItem::Lifecycle(_) | EventSubscriptionItem::Event { .. } => {}
        }
    }
    assert!(saw_rebinding, "old stream must observe rebinding lifecycle");
    assert!(saw_closed, "old stream must observe closed lifecycle");
    assert!(matches!(
        timeout(Duration::from_secs(2), old_stream.recv()).await,
        Ok(Err(tokio::sync::broadcast::error::RecvError::Closed))
    ));

    let mut new_stream = service_b.subscribe_events().await?;
    let new_session_id = new_stream.session_id();
    assert!(new_session_id > old_session_id);
    assert_eq!(
        new_stream.recv().await?,
        EventSubscriptionItem::Lifecycle(SubscriptionLifecycle::Opened {
            session_id: new_session_id,
        })
    );

    let reconnect_deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < reconnect_deadline {
        if !service_a.connected_peers().await?.is_empty()
            && !service_b.connected_peers().await?.is_empty()
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(
        !service_a.connected_peers().await?.is_empty(),
        "service A must reconnect to service B after restart"
    );
    assert!(
        !service_b.connected_peers().await?.is_empty(),
        "service B must reconnect to service A after restart"
    );

    let marker = format!("post-restart-{new_session_id}");
    service_a
        .apply_local_clipboard_change(LocalClipboardChangeRequest {
            text: marker.clone(),
            targets: Targets::all(),
        })
        .await?;

    let text_deadline = Instant::now() + Duration::from_secs(10);
    let mut saw_text = false;
    while Instant::now() < text_deadline {
        let remain = text_deadline.saturating_duration_since(Instant::now());
        if remain.is_zero() {
            break;
        }

        let item = timeout(remain, new_stream.recv()).await??;
        if let EventSubscriptionItem::Event {
            session_id,
            event: AppEvent::Sync(SyncEvent::TextReceived { content, .. }),
        } = item
            && session_id == new_session_id
            && content == marker
        {
            saw_text = true;
            break;
        }
    }
    assert!(saw_text, "new stream must continue receiving sync events");

    service_a.stop_engine().await?;
    service_b.stop_engine().await?;
    Ok(())
}
