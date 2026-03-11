use std::path::Path;

use crate::defaults::{
    default_active_downloads, default_chunk_size, default_config_version,
    default_connect_timeout_ms, default_decision_timeout_ms, default_dedup_window_days,
    default_gc_batch_size, default_gc_every_inserts, default_handshake_timeout_ms,
    default_history_window_days, default_idle_timeout_ms, default_listen_addr,
    default_local_capture_enabled, default_max_file_size, default_max_packet_size,
    default_max_text_bytes, default_mdns_enabled, default_network_enabled,
    default_ping_interval_ms, default_pong_timeout_ms, default_recent_event_lookup_limit,
};
use crate::schema::{
    AppConfig, AppSection, ClipboardAppConfig, IdentityConfig, MetaConfig, StorageLifecycleConfig,
    StorageSection, SyncAuthConfig, SyncFileConfig, SyncNetworkConfig, SyncSection,
    SyncTransportConfig,
};
use crate::{ConfigError, ConfigResult};

use super::paths::default_download_dir;
use super::spec::ConfigTemplate;

pub fn write_config_template(path: impl AsRef<Path>, template: ConfigTemplate) -> ConfigResult<()> {
    let path = path.as_ref();
    let config = match template {
        ConfigTemplate::Production => production_template(path)?,
        ConfigTemplate::Development => development_template(path)?,
    };
    config.save_atomically(path)
}

fn production_template(path: &Path) -> ConfigResult<AppConfig> {
    let _parent = path.parent().ok_or_else(|| {
        ConfigError::InvalidConfig(format!("config path `{}` has no parent", path.display()))
    })?;

    Ok(AppConfig {
        meta: MetaConfig {
            config_version: default_config_version(),
            profile: "production".to_string(),
        },
        identity: IdentityConfig {
            noob_id_file: "noob_id".into(),
            device_id: default_device_id(),
        },
        app: AppSection {
            clipboard: ClipboardAppConfig {
                recent_event_lookup_limit: default_recent_event_lookup_limit(),
                local_capture_enabled: default_local_capture_enabled(),
            },
        },
        storage: StorageSection {
            db_root: "data".into(),
            max_text_bytes: default_max_text_bytes(),
            retain_old_versions: 0,
            lifecycle: StorageLifecycleConfig {
                history_window_days: default_history_window_days(),
                dedup_window_days: default_dedup_window_days(),
                gc_every_inserts: default_gc_every_inserts(),
                gc_batch_size: default_gc_batch_size(),
            },
        },
        sync: SyncSection {
            network: SyncNetworkConfig {
                enabled: default_network_enabled(),
                mdns_enabled: default_mdns_enabled(),
                listen_addr: default_listen_addr(),
                manual_peers: Vec::new(),
            },
            auth: SyncAuthConfig {
                token: uuid::Uuid::now_v7().to_string(),
            },
            file: SyncFileConfig {
                download_dir: default_download_dir()?,
                max_file_size: default_max_file_size(),
                chunk_size: default_chunk_size(),
                active_downloads: default_active_downloads(),
                decision_timeout_ms: default_decision_timeout_ms(),
                idle_timeout_ms: default_idle_timeout_ms(),
            },
            transport: SyncTransportConfig {
                connect_timeout_ms: default_connect_timeout_ms(),
                handshake_timeout_ms: default_handshake_timeout_ms(),
                ping_interval_ms: default_ping_interval_ms(),
                pong_timeout_ms: default_pong_timeout_ms(),
                max_packet_size: default_max_packet_size(),
            },
        },
        noob_id: None,
    })
}

fn development_template(path: &Path) -> ConfigResult<AppConfig> {
    let _parent = path.parent().ok_or_else(|| {
        ConfigError::InvalidConfig(format!("config path `{}` has no parent", path.display()))
    })?;

    Ok(AppConfig {
        meta: MetaConfig {
            config_version: default_config_version(),
            profile: "dev".to_string(),
        },
        identity: IdentityConfig {
            noob_id_file: ".dev-data/noob_id".into(),
            device_id: "dev-device".to_string(),
        },
        app: AppSection {
            clipboard: ClipboardAppConfig {
                recent_event_lookup_limit: default_recent_event_lookup_limit(),
                local_capture_enabled: true,
            },
        },
        storage: StorageSection {
            db_root: ".dev-data".into(),
            max_text_bytes: default_max_text_bytes(),
            retain_old_versions: 0,
            lifecycle: StorageLifecycleConfig {
                history_window_days: default_history_window_days(),
                dedup_window_days: default_dedup_window_days(),
                gc_every_inserts: default_gc_every_inserts(),
                gc_batch_size: default_gc_batch_size(),
            },
        },
        sync: SyncSection {
            network: SyncNetworkConfig {
                enabled: default_network_enabled(),
                mdns_enabled: default_mdns_enabled(),
                listen_addr: default_listen_addr(),
                manual_peers: Vec::new(),
            },
            auth: SyncAuthConfig {
                token: "dev-sync-token".to_string(),
            },
            file: SyncFileConfig {
                download_dir: ".dev-data/downloads".into(),
                max_file_size: default_max_file_size(),
                chunk_size: default_chunk_size(),
                active_downloads: default_active_downloads(),
                decision_timeout_ms: default_decision_timeout_ms(),
                idle_timeout_ms: default_idle_timeout_ms(),
            },
            transport: SyncTransportConfig {
                connect_timeout_ms: default_connect_timeout_ms(),
                handshake_timeout_ms: default_handshake_timeout_ms(),
                ping_interval_ms: default_ping_interval_ms(),
                pong_timeout_ms: default_pong_timeout_ms(),
                max_packet_size: default_max_packet_size(),
            },
        },
        noob_id: None,
    })
}

fn default_device_id() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "my-device".to_string())
}
