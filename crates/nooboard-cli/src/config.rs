use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use nooboard_core::NooboardError;
use nooboard_sync::SyncConfig;
use serde::Deserialize;
use uuid::Uuid;

#[derive(Debug, Deserialize, Default)]
struct CliConfig {
    #[serde(default)]
    sync: SyncSection,
    #[serde(default)]
    identity: IdentitySection,
}

#[derive(Debug, Deserialize, Default)]
struct IdentitySection {
    noob_id_file: Option<PathBuf>,
    device_id: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct SyncSection {
    #[serde(default)]
    enabled: bool,
    #[serde(default = "default_true")]
    mdns_enabled: bool,
    listen_addr: Option<String>,
    token: Option<String>,
    #[serde(default)]
    manual_peers: Vec<String>,
    max_packet_size: Option<usize>,
    file_chunk_size: Option<usize>,
    file_decision_timeout_ms: Option<u64>,
    transfer_idle_timeout_ms: Option<u64>,
    download_dir: Option<PathBuf>,
    max_file_size: Option<u64>,
    connect_timeout_ms: Option<u64>,
    handshake_timeout_ms: Option<u64>,
    ping_interval_ms: Option<u64>,
    pong_timeout_ms: Option<u64>,
    active_downloads: Option<usize>,
}

pub fn load_sync_config(config_path: &Path) -> Result<Option<SyncConfig>, NooboardError> {
    let raw = fs::read_to_string(config_path)?;
    let parsed: CliConfig = toml::from_str(&raw)
        .map_err(|error| NooboardError::storage(format!("failed to parse sync config: {error}")))?;

    if !parsed.sync.enabled {
        return Ok(None);
    }

    let base_dir = config_path.parent().unwrap_or_else(|| Path::new("."));
    let noob_id_file = parsed
        .identity
        .noob_id_file
        .as_ref()
        .map(|path| absolutize(path, base_dir))
        .unwrap_or_else(default_noob_id_file);
    let noob_id = ensure_noob_id(&noob_id_file)?;
    let device_id = resolve_device_id(&parsed.identity, &noob_id);

    let download_dir = parsed
        .sync
        .download_dir
        .as_ref()
        .map(|path| absolutize(path, base_dir))
        .unwrap_or_else(default_download_dir);
    fs::create_dir_all(&download_dir)?;

    let listen_addr = parsed
        .sync
        .listen_addr
        .as_deref()
        .unwrap_or("0.0.0.0:17890")
        .parse::<SocketAddr>()
        .map_err(|error| NooboardError::storage(format!("invalid sync.listen_addr: {error}")))?;

    let manual_peers = parsed
        .sync
        .manual_peers
        .iter()
        .map(|value| {
            value.parse::<SocketAddr>().map_err(|error| {
                NooboardError::storage(format!(
                    "invalid sync.manual_peers entry `{value}`: {error}"
                ))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    let token = parsed
        .sync
        .token
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| NooboardError::storage("sync.token must not be empty".to_string()))?;

    let mut config = SyncConfig {
        enabled: true,
        mdns_enabled: parsed.sync.mdns_enabled,
        listen_addr,
        token,
        manual_peers,
        protocol_version: nooboard_sync::protocol::PROTOCOL_VERSION,
        connect_timeout_ms: parsed.sync.connect_timeout_ms.unwrap_or(5_000),
        handshake_timeout_ms: parsed.sync.handshake_timeout_ms.unwrap_or(5_000),
        ping_interval_ms: parsed.sync.ping_interval_ms.unwrap_or(5_000),
        pong_timeout_ms: parsed.sync.pong_timeout_ms.unwrap_or(15_000),
        max_packet_size: parsed.sync.max_packet_size.unwrap_or(8 * 1024 * 1024),
        file_chunk_size: parsed.sync.file_chunk_size.unwrap_or(64 * 1024),
        file_decision_timeout_ms: parsed.sync.file_decision_timeout_ms.unwrap_or(30_000),
        transfer_idle_timeout_ms: parsed.sync.transfer_idle_timeout_ms.unwrap_or(15_000),
        download_dir,
        max_file_size: parsed.sync.max_file_size.unwrap_or(10 * 1024 * 1024 * 1024),
        active_downloads: parsed.sync.active_downloads.unwrap_or(8),
        noob_id,
        device_id,
    };

    if let Err(message) = config.validate() {
        return Err(NooboardError::storage(message));
    }

    config.enabled = true;
    Ok(Some(config))
}

fn ensure_noob_id(path: &Path) -> Result<String, NooboardError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    if path.exists() {
        let existing = fs::read_to_string(path)?;
        let value = existing.trim().to_string();
        if !value.is_empty() {
            return Ok(value);
        }
    }

    let generated = Uuid::now_v7().to_string();
    fs::write(path, format!("{generated}\n"))?;
    Ok(generated)
}

fn default_noob_id_file() -> PathBuf {
    home_dir().join(".nooboard").join("noob_id")
}

fn resolve_device_id(identity: &IdentitySection, noob_id: &str) -> String {
    identity
        .device_id
        .as_ref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            std::env::var("NOOBOARD_DEVICE_ID")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        })
        .or_else(|| {
            std::env::var("HOSTNAME")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        })
        .unwrap_or_else(|| {
            let suffix_len = noob_id.len().min(8);
            format!("device-{}", &noob_id[..suffix_len])
        })
}

fn default_download_dir() -> PathBuf {
    home_dir().join("Downloads").join("nooboard")
}

fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn absolutize(path: &Path, base_dir: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base_dir.join(path)
    }
}

const fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_noob_id_when_file_missing() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let config_path = temp.path().join("dev.toml");
        let noob_id_path = temp.path().join("identity").join("noob_id");

        let raw = format!(
            "\
[sync]
enabled = true
listen_addr = \"127.0.0.1:19001\"
token = \"abc\"
manual_peers = []

[identity]
noob_id_file = \"{}\"
",
            noob_id_path.display()
        );
        fs::write(&config_path, raw)?;

        let loaded = load_sync_config(&config_path)?;
        let loaded = loaded.expect("sync config must be enabled");
        assert!(!loaded.noob_id.trim().is_empty());
        assert!(!loaded.device_id.trim().is_empty());
        assert!(noob_id_path.exists());

        Ok(())
    }

    #[test]
    fn uses_configured_device_id_when_provided() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let config_path = temp.path().join("dev.toml");

        let raw = "\
[sync]
enabled = true
listen_addr = \"127.0.0.1:19001\"
token = \"abc\"
manual_peers = []

[identity]
device_id = \"Alice MacBook\"
";
        fs::write(&config_path, raw)?;

        let loaded = load_sync_config(&config_path)?;
        let loaded = loaded.expect("sync config must be enabled");
        assert_eq!(loaded.device_id, "Alice MacBook");
        Ok(())
    }
}
