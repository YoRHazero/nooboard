use std::net::SocketAddr;
use std::path::PathBuf;

use crate::protocol::PROTOCOL_VERSION;

#[derive(Debug, Clone)]
pub struct SyncConfig {
    pub enabled: bool,
    pub mdns_enabled: bool,
    pub listen_addr: SocketAddr,
    pub token: String,
    pub manual_peers: Vec<SocketAddr>,
    pub protocol_version: u16,
    pub connect_timeout_ms: u64,
    pub handshake_timeout_ms: u64,
    pub ping_interval_ms: u64,
    pub pong_timeout_ms: u64,
    pub max_packet_size: usize,
    pub file_chunk_size: usize,
    pub file_decision_timeout_ms: u64,
    pub transfer_idle_timeout_ms: u64,
    pub download_dir: PathBuf,
    pub max_file_size: u64,
    pub active_downloads: usize,
    pub noob_id: String,
    pub device_id: String,
}

impl SyncConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.token.trim().is_empty() {
            return Err("sync.token must not be empty".to_string());
        }

        if self.noob_id.trim().is_empty() {
            return Err("sync.noob_id must not be empty".to_string());
        }

        if self.device_id.trim().is_empty() {
            return Err("sync.device_id must not be empty".to_string());
        }

        if self.max_packet_size == 0 {
            return Err("sync.max_packet_size must be > 0".to_string());
        }

        if self.file_chunk_size == 0 {
            return Err("sync.file_chunk_size must be > 0".to_string());
        }

        if self.file_chunk_size > self.max_packet_size {
            return Err("sync.file_chunk_size must be <= sync.max_packet_size".to_string());
        }

        if self.file_decision_timeout_ms == 0 {
            return Err("sync.file_decision_timeout_ms must be > 0".to_string());
        }

        if self.transfer_idle_timeout_ms == 0 {
            return Err("sync.transfer_idle_timeout_ms must be > 0".to_string());
        }

        if self.max_file_size == 0 {
            return Err("sync.max_file_size must be > 0".to_string());
        }

        if self.active_downloads == 0 {
            return Err("sync.active_downloads must be > 0".to_string());
        }

        if self.protocol_version != PROTOCOL_VERSION {
            return Err(format!(
                "sync.protocol_version={} must match protocol version {}",
                self.protocol_version, PROTOCOL_VERSION
            ));
        }

        Ok(())
    }
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            mdns_enabled: true,
            listen_addr: "127.0.0.1:0"
                .parse()
                .expect("default listen addr must be valid"),
            token: "nooboard-dev-token".to_string(),
            manual_peers: Vec::new(),
            protocol_version: PROTOCOL_VERSION,
            connect_timeout_ms: 5_000,
            handshake_timeout_ms: 5_000,
            ping_interval_ms: 5_000,
            pong_timeout_ms: 15_000,
            max_packet_size: 8 * 1024 * 1024,
            file_chunk_size: 64 * 1024,
            file_decision_timeout_ms: 30_000,
            transfer_idle_timeout_ms: 15_000,
            download_dir: std::env::temp_dir().join("nooboard-downloads"),
            max_file_size: 10 * 1024 * 1024 * 1024,
            active_downloads: 8,
            noob_id: "dev-node".to_string(),
            device_id: "dev-device".to_string(),
        }
    }
}
