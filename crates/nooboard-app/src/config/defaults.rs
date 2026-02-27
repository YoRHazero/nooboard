use std::net::SocketAddr;
use std::path::PathBuf;

pub const APP_CONFIG_VERSION: u32 = 2;
pub const DEFAULT_RECENT_EVENT_LOOKUP_LIMIT: usize = 50;

pub(super) fn default_config_version() -> u32 {
    APP_CONFIG_VERSION
}

pub(super) fn default_profile() -> String {
    "dev".to_string()
}

pub(super) fn default_recent_event_lookup_limit() -> usize {
    DEFAULT_RECENT_EVENT_LOOKUP_LIMIT
}

pub(super) fn default_history_window_days() -> u32 {
    7
}

pub(super) fn default_dedup_window_days() -> u32 {
    14
}

pub(super) fn default_gc_every_inserts() -> u32 {
    200
}

pub(super) fn default_gc_batch_size() -> u32 {
    500
}

pub(super) fn default_network_enabled() -> bool {
    true
}

pub(super) fn default_mdns_enabled() -> bool {
    true
}

pub(super) fn default_listen_addr() -> SocketAddr {
    "0.0.0.0:17890"
        .parse()
        .expect("default sync listen addr must parse")
}

pub(super) fn default_sync_token() -> String {
    "dev-sync-token".to_string()
}

pub(super) fn default_download_dir() -> PathBuf {
    std::env::temp_dir().join("nooboard-downloads")
}

pub(super) fn default_max_file_size() -> u64 {
    10 * 1024 * 1024 * 1024
}

pub(super) fn default_chunk_size() -> usize {
    64 * 1024
}

pub(super) fn default_active_downloads() -> usize {
    8
}

pub(super) fn default_decision_timeout_ms() -> u64 {
    30_000
}

pub(super) fn default_idle_timeout_ms() -> u64 {
    15_000
}

pub(super) fn default_connect_timeout_ms() -> u64 {
    5_000
}

pub(super) fn default_handshake_timeout_ms() -> u64 {
    5_000
}

pub(super) fn default_ping_interval_ms() -> u64 {
    5_000
}

pub(super) fn default_pong_timeout_ms() -> u64 {
    15_000
}

pub(super) fn default_max_packet_size() -> usize {
    8 * 1024 * 1024
}
