use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{DirectSeedId, NetworkError, NetworkResult};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub identity: LocalIdentityConfig,
    pub listen_port: u16,
    pub auth: NetworkAuthConfig,
    pub lan: LanConfig,
    pub direct: DirectConfig,
    pub transport: NetworkTransportConfig,
    pub transfer: NetworkTransferConfig,
}

impl NetworkConfig {
    pub fn validate(&self) -> NetworkResult<()> {
        if self.identity.noob_id.trim().is_empty() {
            return Err(NetworkError::InvalidConfig(
                "identity.noob_id must not be empty".to_string(),
            ));
        }
        if self.identity.device_id.trim().is_empty() {
            return Err(NetworkError::InvalidConfig(
                "identity.device_id must not be empty".to_string(),
            ));
        }
        if self.listen_port == 0 {
            return Err(NetworkError::InvalidConfig(
                "listen_port must be > 0".to_string(),
            ));
        }
        if self.auth.token.trim().is_empty() {
            return Err(NetworkError::InvalidConfig(
                "auth.token must not be empty".to_string(),
            ));
        }
        if self.direct.approval_timeout_ms == 0 {
            return Err(NetworkError::InvalidConfig(
                "direct.approval_timeout_ms must be > 0".to_string(),
            ));
        }
        if self.transport.connect_timeout_ms == 0 {
            return Err(NetworkError::InvalidConfig(
                "transport.connect_timeout_ms must be > 0".to_string(),
            ));
        }
        if self.transport.handshake_timeout_ms == 0 {
            return Err(NetworkError::InvalidConfig(
                "transport.handshake_timeout_ms must be > 0".to_string(),
            ));
        }
        if self.transport.ping_interval_ms == 0 {
            return Err(NetworkError::InvalidConfig(
                "transport.ping_interval_ms must be > 0".to_string(),
            ));
        }
        if self.transport.pong_timeout_ms == 0 {
            return Err(NetworkError::InvalidConfig(
                "transport.pong_timeout_ms must be > 0".to_string(),
            ));
        }
        if self.transport.max_packet_size == 0 {
            return Err(NetworkError::InvalidConfig(
                "transport.max_packet_size must be > 0".to_string(),
            ));
        }
        if self.transfer.max_file_size == 0 {
            return Err(NetworkError::InvalidConfig(
                "transfer.max_file_size must be > 0".to_string(),
            ));
        }
        if self.transfer.chunk_size == 0 {
            return Err(NetworkError::InvalidConfig(
                "transfer.chunk_size must be > 0".to_string(),
            ));
        }
        if self.transfer.active_downloads == 0 {
            return Err(NetworkError::InvalidConfig(
                "transfer.active_downloads must be > 0".to_string(),
            ));
        }
        if self.transfer.decision_timeout_ms == 0 {
            return Err(NetworkError::InvalidConfig(
                "transfer.decision_timeout_ms must be > 0".to_string(),
            ));
        }
        if self.transfer.idle_timeout_ms == 0 {
            return Err(NetworkError::InvalidConfig(
                "transfer.idle_timeout_ms must be > 0".to_string(),
            ));
        }
        if self.transfer.chunk_size > self.transport.max_packet_size {
            return Err(NetworkError::InvalidConfig(
                "transfer.chunk_size must be <= transport.max_packet_size".to_string(),
            ));
        }

        let mut seen = std::collections::HashSet::new();
        for seed in &self.direct.seeds {
            seed.validate()?;
            if !seen.insert(seed.id) {
                return Err(NetworkError::InvalidConfig(format!(
                    "direct.seeds contains duplicate id {}",
                    seed.id
                )));
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalIdentityConfig {
    pub noob_id: String,
    pub device_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkAuthConfig {
    pub token: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanConfig {
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirectConfig {
    pub approval_timeout_ms: u64,
    pub seeds: Vec<DirectSeedConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirectSeedConfig {
    pub id: Uuid,
    pub label: String,
    pub host: String,
    pub port: u16,
    pub enabled: bool,
}

impl DirectSeedConfig {
    pub fn validate(&self) -> NetworkResult<()> {
        if self.host.trim().is_empty() {
            return Err(NetworkError::InvalidConfig(format!(
                "direct seed {} host must not be empty",
                self.id
            )));
        }
        if self.port == 0 {
            return Err(NetworkError::InvalidConfig(format!(
                "direct seed {} port must be > 0",
                self.id
            )));
        }
        Ok(())
    }

    pub fn public_id(&self) -> DirectSeedId {
        DirectSeedId::from_uuid(self.id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkTransportConfig {
    pub connect_timeout_ms: u64,
    pub handshake_timeout_ms: u64,
    pub ping_interval_ms: u64,
    pub pong_timeout_ms: u64,
    pub max_packet_size: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkTransferConfig {
    pub download_dir: PathBuf,
    pub max_file_size: u64,
    pub chunk_size: usize,
    pub active_downloads: usize,
    pub decision_timeout_ms: u64,
    pub idle_timeout_ms: u64,
}
