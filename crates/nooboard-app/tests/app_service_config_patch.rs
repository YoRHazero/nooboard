use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use nooboard_app::{
    AppConfig, AppError, AppResult, AppService, AppServiceImpl, AppSyncStatus, ClipboardPort,
    EventId, ListHistoryRequest, NetworkPatch, RemoteTextRequest, StoragePatch,
};
use tempfile::TempDir;

#[derive(Default)]
struct MockClipboardBackend {
    text: Mutex<Option<String>>,
}

impl ClipboardPort for MockClipboardBackend {
    fn read_text(&self) -> AppResult<Option<String>> {
        Ok(self.text.lock().ok().and_then(|value| value.clone()))
    }

    fn write_text(&self, text: &str) -> AppResult<()> {
        if let Ok(mut value) = self.text.lock() {
            *value = Some(text.to_string());
        }
        Ok(())
    }
}

fn toml_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "\\\\")
}

fn write_test_config(dir: &TempDir, db_root: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let config_path = dir.path().join("app.toml");
    let noob_id_file = dir.path().join("node_id");
    let download_dir = dir.path().join("downloads");
    let listen_addr: SocketAddr = "127.0.0.1:0".parse()?;

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
        db_root = toml_path(db_root),
        listen_addr = listen_addr,
        download_dir = toml_path(&download_dir),
    );

    std::fs::write(&config_path, raw)?;
    Ok(config_path)
}

fn new_service(
    db_root: &Path,
) -> Result<(AppServiceImpl, TempDir, PathBuf), Box<dyn std::error::Error>> {
    let dir = TempDir::new()?;
    let config_path = write_test_config(&dir, db_root)?;
    let backend = Arc::new(MockClipboardBackend::default());
    let service = AppServiceImpl::new(&config_path, backend)?;
    Ok((service, dir, config_path))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn storage_patch_switches_active_database() -> Result<(), Box<dyn std::error::Error>> {
    let root = TempDir::new()?;
    let db_a = root.path().join("db-a");
    let db_b = root.path().join("db-b");
    let (service, _dir, config_path) = new_service(&db_a)?;

    service
        .store_remote_text(RemoteTextRequest {
            event_id: EventId::new(),
            content: "from-a".to_string(),
            device_id: "remote-a".to_string(),
        })
        .await?;
    let before = service
        .list_history(ListHistoryRequest {
            limit: 10,
            cursor: None,
        })
        .await?;
    assert_eq!(before.records.len(), 1);
    assert_eq!(before.records[0].content, "from-a");

    let applied = service
        .apply_storage_patch(StoragePatch {
            db_root: Some(db_b.clone()),
            ..StoragePatch::default()
        })
        .await?;
    assert_eq!(applied.db_root, db_b);

    let after_switch = service
        .list_history(ListHistoryRequest {
            limit: 10,
            cursor: None,
        })
        .await?;
    assert!(after_switch.records.is_empty());

    service
        .store_remote_text(RemoteTextRequest {
            event_id: EventId::new(),
            content: "from-b".to_string(),
            device_id: "remote-b".to_string(),
        })
        .await?;
    let b_records = service
        .list_history(ListHistoryRequest {
            limit: 10,
            cursor: None,
        })
        .await?;
    assert_eq!(b_records.records.len(), 1);
    assert_eq!(b_records.records[0].content, "from-b");

    let persisted = AppConfig::load(config_path)?;
    assert_eq!(persisted.storage.db_root, db_b);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn storage_patch_invalid_values_are_rejected_without_persisting()
-> Result<(), Box<dyn std::error::Error>> {
    let root = TempDir::new()?;
    let db_root = root.path().join("db");
    let (service, _dir, config_path) = new_service(&db_root)?;

    let result = service
        .apply_storage_patch(StoragePatch {
            history_window_days: Some(30),
            dedup_window_days: Some(7),
            ..StoragePatch::default()
        })
        .await;
    assert!(matches!(result, Err(AppError::InvalidConfig(_))));

    let persisted = AppConfig::load(config_path)?;
    assert_eq!(persisted.storage.lifecycle.history_window_days, 7);
    assert_eq!(persisted.storage.lifecycle.dedup_window_days, 14);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn storage_patch_resolves_relative_db_root_from_config_dir()
-> Result<(), Box<dyn std::error::Error>> {
    let root = TempDir::new()?;
    let db_a = root.path().join("db-a");
    let (service, _dir, config_path) = new_service(&db_a)?;

    let relative_db_root = PathBuf::from("db-relative");
    let expected_db_root = config_path
        .parent()
        .expect("config path must have parent")
        .join(&relative_db_root);

    let applied = service
        .apply_storage_patch(StoragePatch {
            db_root: Some(relative_db_root),
            ..StoragePatch::default()
        })
        .await?;
    assert_eq!(applied.db_root, expected_db_root);

    service
        .store_remote_text(RemoteTextRequest {
            event_id: EventId::new(),
            content: "from-relative".to_string(),
            device_id: "remote-a".to_string(),
        })
        .await?;

    let before_restart = service
        .list_history(ListHistoryRequest {
            limit: 10,
            cursor: None,
        })
        .await?;
    assert_eq!(before_restart.records.len(), 1);
    assert_eq!(before_restart.records[0].content, "from-relative");

    service.restart_sync_engine().await?;
    let after_restart = service
        .list_history(ListHistoryRequest {
            limit: 10,
            cursor: None,
        })
        .await?;
    assert_eq!(after_restart.records.len(), 1);
    assert_eq!(after_restart.records[0].content, "from-relative");

    let persisted = AppConfig::load(config_path)?;
    assert_eq!(persisted.storage.db_root, expected_db_root);

    service.stop_sync_engine().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn network_patch_does_not_reconfigure_storage_runtime()
-> Result<(), Box<dyn std::error::Error>> {
    let root = TempDir::new()?;
    let db_a = root.path().join("db-a");
    let db_b = root.path().join("db-b");
    let (service, _dir, config_path) = new_service(&db_a)?;

    service
        .store_remote_text(RemoteTextRequest {
            event_id: EventId::new(),
            content: "from-a".to_string(),
            device_id: "remote-a".to_string(),
        })
        .await?;

    let mut external_config = AppConfig::load(&config_path)?;
    external_config.storage.db_root = db_b;
    external_config.save_atomically(&config_path)?;

    let _ = service
        .apply_network_patch(NetworkPatch::SetMdnsEnabled(false))
        .await?;

    let records = service
        .list_history(ListHistoryRequest {
            limit: 10,
            cursor: None,
        })
        .await?;
    assert_eq!(records.records.len(), 1);
    assert_eq!(records.records[0].content, "from-a");
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn restart_engine_rolls_back_storage_when_sync_restart_fails()
-> Result<(), Box<dyn std::error::Error>> {
    let root = TempDir::new()?;
    let db_a = root.path().join("db-a");
    let db_b = root.path().join("db-b");
    let (service, _dir, config_path) = new_service(&db_a)?;

    service.start_sync_engine().await?;
    service
        .store_remote_text(RemoteTextRequest {
            event_id: EventId::new(),
            content: "from-a".to_string(),
            device_id: "remote-a".to_string(),
        })
        .await?;

    let blocked_download_dir = root.path().join("blocked-download-dir");
    std::fs::write(&blocked_download_dir, b"occupied-by-file")?;

    let mut external_config = AppConfig::load(&config_path)?;
    external_config.storage.db_root = db_b;
    external_config.sync.file.download_dir = blocked_download_dir;
    external_config.save_atomically(&config_path)?;

    let restart_result = service.restart_sync_engine().await;
    assert!(
        matches!(restart_result, Err(AppError::Sync(_))),
        "unexpected restart result: {restart_result:?}"
    );

    service
        .store_remote_text(RemoteTextRequest {
            event_id: EventId::new(),
            content: "after-failed-restart".to_string(),
            device_id: "remote-b".to_string(),
        })
        .await?;

    let records = service
        .list_history(ListHistoryRequest {
            limit: 10,
            cursor: None,
        })
        .await?;
    let contents: Vec<&str> = records
        .records
        .iter()
        .map(|record| record.content.as_str())
        .collect();
    assert!(contents.contains(&"from-a"));
    assert!(contents.contains(&"after-failed-restart"));

    let status = service.sync_status().await?;
    assert!(matches!(
        status,
        AppSyncStatus::Starting | AppSyncStatus::Running
    ));

    service.stop_sync_engine().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn start_sync_engine_returns_already_running_without_restarting()
-> Result<(), Box<dyn std::error::Error>> {
    let root = TempDir::new()?;
    let db_a = root.path().join("db-a");
    let (service, _dir, _config_path) = new_service(&db_a)?;

    service.start_sync_engine().await?;

    let start_result = service.start_sync_engine().await;
    assert!(matches!(start_result, Err(AppError::EngineAlreadyRunning)));

    service
        .store_remote_text(RemoteTextRequest {
            event_id: EventId::new(),
            content: "after-already-running".to_string(),
            device_id: "remote-a".to_string(),
        })
        .await?;
    let records = service
        .list_history(ListHistoryRequest {
            limit: 10,
            cursor: None,
        })
        .await?;
    assert!(
        records
            .records
            .iter()
            .any(|record| record.content == "after-already-running")
    );

    let status = service.sync_status().await?;
    assert!(matches!(
        status,
        AppSyncStatus::Starting | AppSyncStatus::Running
    ));

    service.stop_sync_engine().await?;
    Ok(())
}
