use std::net::SocketAddr;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::defaults::{
    default_active_downloads, default_chunk_size, default_config_version,
    default_connect_timeout_ms, default_decision_timeout_ms, default_dedup_window_days,
    default_download_dir, default_gc_batch_size, default_gc_every_inserts,
    default_handshake_timeout_ms, default_history_window_days, default_idle_timeout_ms,
    default_listen_addr, default_max_file_size, default_max_packet_size, default_mdns_enabled,
    default_network_enabled, default_ping_interval_ms, default_pong_timeout_ms, default_profile,
    default_recent_event_lookup_limit, default_sync_token,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub meta: MetaConfig,
    pub identity: IdentityConfig,
    #[serde(default)]
    pub app: AppSection,
    pub storage: StorageSection,
    pub sync: SyncSection,
    #[serde(skip)]
    pub(super) noob_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetaConfig {
    #[serde(default = "default_config_version")]
    pub config_version: u32,
    #[serde(default = "default_profile")]
    pub profile: String,
}

impl Default for MetaConfig {
    fn default() -> Self {
        Self {
            config_version: default_config_version(),
            profile: default_profile(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityConfig {
    pub noob_id_file: PathBuf,
    pub device_id: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppSection {
    #[serde(default)]
    pub clipboard: ClipboardAppConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardAppConfig {
    #[serde(default = "default_recent_event_lookup_limit")]
    pub recent_event_lookup_limit: usize,
}

impl Default for ClipboardAppConfig {
    fn default() -> Self {
        Self {
            recent_event_lookup_limit: default_recent_event_lookup_limit(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageSection {
    pub db_root: PathBuf,
    #[serde(default)]
    pub retain_old_versions: usize,
    #[serde(default)]
    pub lifecycle: StorageLifecycleConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageLifecycleConfig {
    #[serde(default = "default_history_window_days")]
    pub history_window_days: u32,
    #[serde(default = "default_dedup_window_days")]
    pub dedup_window_days: u32,
    #[serde(default = "default_gc_every_inserts")]
    pub gc_every_inserts: u32,
    #[serde(default = "default_gc_batch_size")]
    pub gc_batch_size: u32,
}

impl Default for StorageLifecycleConfig {
    fn default() -> Self {
        Self {
            history_window_days: default_history_window_days(),
            dedup_window_days: default_dedup_window_days(),
            gc_every_inserts: default_gc_every_inserts(),
            gc_batch_size: default_gc_batch_size(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SyncSection {
    #[serde(default)]
    pub network: SyncNetworkConfig,
    #[serde(default)]
    pub auth: SyncAuthConfig,
    #[serde(default)]
    pub file: SyncFileConfig,
    #[serde(default)]
    pub transport: SyncTransportConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncNetworkConfig {
    #[serde(default = "default_network_enabled")]
    pub enabled: bool,
    #[serde(default = "default_mdns_enabled")]
    pub mdns_enabled: bool,
    #[serde(default = "default_listen_addr")]
    pub listen_addr: SocketAddr,
    #[serde(default)]
    pub manual_peers: Vec<SocketAddr>,
}

impl Default for SyncNetworkConfig {
    fn default() -> Self {
        Self {
            enabled: default_network_enabled(),
            mdns_enabled: default_mdns_enabled(),
            listen_addr: default_listen_addr(),
            manual_peers: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncAuthConfig {
    #[serde(default = "default_sync_token")]
    pub token: String,
}

impl Default for SyncAuthConfig {
    fn default() -> Self {
        Self {
            token: default_sync_token(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncFileConfig {
    #[serde(default = "default_download_dir")]
    pub download_dir: PathBuf,
    #[serde(default = "default_max_file_size")]
    pub max_file_size: u64,
    #[serde(default = "default_chunk_size")]
    pub chunk_size: usize,
    #[serde(default = "default_active_downloads")]
    pub active_downloads: usize,
    #[serde(default = "default_decision_timeout_ms")]
    pub decision_timeout_ms: u64,
    #[serde(default = "default_idle_timeout_ms")]
    pub idle_timeout_ms: u64,
}

impl Default for SyncFileConfig {
    fn default() -> Self {
        Self {
            download_dir: default_download_dir(),
            max_file_size: default_max_file_size(),
            chunk_size: default_chunk_size(),
            active_downloads: default_active_downloads(),
            decision_timeout_ms: default_decision_timeout_ms(),
            idle_timeout_ms: default_idle_timeout_ms(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncTransportConfig {
    #[serde(default = "default_connect_timeout_ms")]
    pub connect_timeout_ms: u64,
    #[serde(default = "default_handshake_timeout_ms")]
    pub handshake_timeout_ms: u64,
    #[serde(default = "default_ping_interval_ms")]
    pub ping_interval_ms: u64,
    #[serde(default = "default_pong_timeout_ms")]
    pub pong_timeout_ms: u64,
    #[serde(default = "default_max_packet_size")]
    pub max_packet_size: usize,
}

impl Default for SyncTransportConfig {
    fn default() -> Self {
        Self {
            connect_timeout_ms: default_connect_timeout_ms(),
            handshake_timeout_ms: default_handshake_timeout_ms(),
            ping_interval_ms: default_ping_interval_ms(),
            pong_timeout_ms: default_pong_timeout_ms(),
            max_packet_size: default_max_packet_size(),
        }
    }
}
