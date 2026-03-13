use nooboard_network::NetworkConfig;
use nooboard_network::{
    DirectConfig, DirectSeedConfig, LanConfig, LocalIdentityConfig, NetworkAuthConfig,
    NetworkTransferConfig, NetworkTransportConfig,
};

use super::schema::AppConfig;
use crate::{ConfigError, ConfigResult};

impl AppConfig {
    pub fn to_storage_config(&self) -> nooboard_storage::AppConfig {
        nooboard_storage::AppConfig {
            storage: nooboard_storage::StorageConfig {
                db_root: self.storage.db_root.clone(),
                retain_old_versions: self.storage.retain_old_versions,
                lifecycle: nooboard_storage::LifecycleConfig {
                    history_window_days: self.storage.lifecycle.history_window_days,
                    dedup_window_days: self.storage.lifecycle.dedup_window_days,
                    gc_every_inserts: self.storage.lifecycle.gc_every_inserts,
                    gc_batch_size: self.storage.lifecycle.gc_batch_size,
                },
            },
        }
    }

    pub fn to_network_config(&self) -> ConfigResult<NetworkConfig> {
        let noob_id = self.noob_id.clone().ok_or_else(|| {
            ConfigError::InvalidConfig("identity.noob_id was not initialized".to_string())
        })?;
        if noob_id.trim().is_empty() {
            return Err(ConfigError::InvalidConfig(
                "identity.noob_id_file produced empty noob_id".to_string(),
            ));
        }

        let network_config = NetworkConfig {
            identity: LocalIdentityConfig {
                noob_id,
                device_id: self.identity.device_id.clone(),
            },
            listen_port: self.network.listen_port,
            auth: NetworkAuthConfig {
                token: self.network.auth.token.clone(),
            },
            lan: LanConfig {
                enabled: self.network.lan.enabled,
            },
            direct: DirectConfig {
                approval_timeout_ms: self.network.direct.approval_timeout_ms,
                seeds: self
                    .network
                    .direct
                    .seeds
                    .iter()
                    .map(|seed| DirectSeedConfig {
                        id: seed.id,
                        label: seed.label.clone(),
                        host: seed.host.clone(),
                        port: seed.port,
                        enabled: seed.enabled,
                    })
                    .collect(),
            },
            transport: NetworkTransportConfig {
                connect_timeout_ms: self.network.transport.connect_timeout_ms,
                handshake_timeout_ms: self.network.transport.handshake_timeout_ms,
                ping_interval_ms: self.network.transport.ping_interval_ms,
                pong_timeout_ms: self.network.transport.pong_timeout_ms,
                max_packet_size: self.network.transport.max_packet_size,
            },
            transfer: NetworkTransferConfig {
                download_dir: self.network.transfer.download_dir.clone(),
                max_file_size: self.network.transfer.max_file_size,
                chunk_size: self.network.transfer.chunk_size,
                active_downloads: self.network.transfer.active_downloads,
                decision_timeout_ms: self.network.transfer.decision_timeout_ms,
                idle_timeout_ms: self.network.transfer.idle_timeout_ms,
            },
        };

        network_config.validate().map_err(|error| {
            ConfigError::InvalidConfig(format!("network config invalid: {error}"))
        })?;
        Ok(network_config)
    }

    pub fn recent_event_lookup_limit(&self) -> usize {
        self.app.clipboard.recent_event_lookup_limit
    }

    pub fn local_capture_enabled(&self) -> bool {
        self.app.clipboard.local_capture_enabled
    }

    pub fn noob_id(&self) -> Option<&str> {
        self.noob_id.as_deref()
    }
}
