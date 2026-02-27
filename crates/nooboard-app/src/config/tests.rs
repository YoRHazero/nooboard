use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::{AppConfig, DEFAULT_RECENT_EVENT_LOOKUP_LIMIT};
use crate::{AppError, AppResult};

fn temp_dir(name: &str) -> PathBuf {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    std::env::temp_dir().join(format!(
        "nooboard-app-config-{name}-{}-{millis}",
        std::process::id()
    ))
}

fn write_base_config(dir: &Path, include_app_clipboard: bool) -> AppResult<PathBuf> {
    fs::create_dir_all(dir)?;
    let config_path = dir.join("app.toml");
    let db_root = dir.join("db");
    let noob_id_file = dir.join("node_id");
    let download_dir = dir.join("downloads");

    let mut raw = format!(
        r#"
[meta]
config_version = 2
profile = "test"

[identity]
noob_id_file = "{noob_id_file}"
device_id = "test-device"

[storage]
db_root = "{db_root}"
retain_old_versions = 0

[storage.lifecycle]
history_window_days = 7
dedup_window_days = 14
gc_every_inserts = 1
gc_batch_size = 1

[sync.network]
enabled = true
mdns_enabled = false
listen_addr = "127.0.0.1:0"
manual_peers = []

[sync.auth]
token = "test-token"

[sync.file]
download_dir = "{download_dir}"
max_file_size = 1024
chunk_size = 128
active_downloads = 1
decision_timeout_ms = 1000
idle_timeout_ms = 1000

[sync.transport]
connect_timeout_ms = 1000
handshake_timeout_ms = 1000
ping_interval_ms = 1000
pong_timeout_ms = 2000
max_packet_size = 4096
"#,
        noob_id_file = noob_id_file.display(),
        db_root = db_root.display(),
        download_dir = download_dir.display(),
    );

    if include_app_clipboard {
        raw.push_str(
            r#"
[app.clipboard]
recent_event_lookup_limit = 25
"#,
        );
    }

    fs::write(&config_path, raw)?;
    Ok(config_path)
}

#[test]
fn load_initializes_node_id_file_when_missing() -> AppResult<()> {
    let dir = temp_dir("node-id-init");
    let config_path = write_base_config(&dir, true)?;

    let config = AppConfig::load(&config_path)?;
    let node_id = config.node_id().unwrap_or_default().to_string();
    assert!(!node_id.is_empty());

    let node_id_file = fs::read_to_string(dir.join("node_id"))?;
    assert_eq!(node_id_file.trim(), node_id);

    let _ = fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn load_uses_default_recent_lookup_limit_when_omitted() -> AppResult<()> {
    let dir = temp_dir("recent-default");
    let config_path = write_base_config(&dir, false)?;

    let config = AppConfig::load(&config_path)?;
    assert_eq!(
        config.recent_event_lookup_limit(),
        DEFAULT_RECENT_EVENT_LOOKUP_LIMIT
    );

    let _ = fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn load_fails_when_existing_node_id_file_is_not_readable_text() -> AppResult<()> {
    let dir = temp_dir("node-id-invalid-utf8");
    let config_path = write_base_config(&dir, true)?;
    let node_id_path = dir.join("node_id");
    fs::write(&node_id_path, [0xFF_u8, 0xFE_u8])?;

    let result = AppConfig::load(&config_path);
    assert!(matches!(result, Err(AppError::Io(_))));

    let written = fs::read(&node_id_path)?;
    assert_eq!(written, vec![0xFF_u8, 0xFE_u8]);

    let _ = fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn regenerate_node_id_recovers_from_corrupted_node_id_file() -> AppResult<()> {
    let dir = temp_dir("node-id-recover");
    let config_path = write_base_config(&dir, true)?;
    let node_id_path = dir.join("node_id");
    fs::write(&node_id_path, [0xFF_u8, 0xFE_u8])?;

    let regenerated = AppConfig::regenerate_node_id(&config_path)?;
    assert!(!regenerated.trim().is_empty());
    assert!(uuid::Uuid::parse_str(&regenerated).is_ok());

    let persisted = fs::read_to_string(&node_id_path)?;
    assert_eq!(persisted.trim(), regenerated);

    let loaded = AppConfig::load(&config_path)?;
    assert_eq!(loaded.node_id(), Some(regenerated.as_str()));

    let _ = fs::remove_dir_all(dir);
    Ok(())
}
