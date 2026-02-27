use crate::config::AppConfig;
use crate::{AppError, AppResult};

use super::{AppServiceImpl, BroadcastConfig, NetworkPatch};

impl AppServiceImpl {
    pub(super) async fn apply_network_patch_usecase(
        &self,
        patch: NetworkPatch,
    ) -> AppResult<BroadcastConfig> {
        match patch {
            NetworkPatch::SetMdnsEnabled(enabled) => {
                self.update_broadcast_config(|config| {
                    config.sync.network.mdns_enabled = enabled;
                    Ok(())
                })
                .await
            }
            NetworkPatch::SetNetworkEnabled(enabled) => {
                self.update_broadcast_config(|config| {
                    config.sync.network.enabled = enabled;
                    Ok(())
                })
                .await
            }
            NetworkPatch::AddManualPeer(addr) => {
                self.update_broadcast_config(|config| {
                    if config.sync.network.manual_peers.contains(&addr) {
                        return Err(AppError::ManualPeerExists {
                            peer: addr.to_string(),
                        });
                    }
                    config.sync.network.manual_peers.push(addr);
                    Ok(())
                })
                .await
            }
            NetworkPatch::RemoveManualPeer(addr) => {
                self.update_broadcast_config(|config| {
                    let before = config.sync.network.manual_peers.len();
                    config
                        .sync
                        .network
                        .manual_peers
                        .retain(|peer| peer != &addr);
                    if config.sync.network.manual_peers.len() == before {
                        return Err(AppError::ManualPeerNotFound {
                            peer: addr.to_string(),
                        });
                    }
                    Ok(())
                })
                .await
            }
        }
    }

    async fn update_broadcast_config<F>(&self, patch: F) -> AppResult<BroadcastConfig>
    where
        F: FnOnce(&mut AppConfig) -> AppResult<()>,
    {
        let _config_guard = self.config_update_lock.lock().await;

        let old_config = AppConfig::load(&self.config_path)?;
        let mut updated_config = old_config.clone();
        patch(&mut updated_config)?;
        updated_config.validate()?;
        let updated_sync_config = updated_config.to_sync_config()?;

        updated_config.save_atomically(&self.config_path)?;

        let restart_result = {
            let mut runtime = self.sync_runtime.lock().await;
            runtime.restart(updated_sync_config).await
        };

        if let Err(restart_error) = restart_result {
            if let Some(rollback_failure) = self
                .rollback_after_restart_failure(&old_config, &restart_error)
                .await
            {
                return Err(rollback_failure);
            }
            return Err(restart_error);
        }

        self.storage_runtime
            .reconfigure(updated_config.to_storage_config())
            .await?;
        *self.config.write().await = updated_config.clone();
        Ok(BroadcastConfig::from_config(&updated_config))
    }

    async fn rollback_after_restart_failure(
        &self,
        old_config: &AppConfig,
        restart_error: &AppError,
    ) -> Option<AppError> {
        if let Err(rollback_write_error) = old_config.save_atomically(&self.config_path) {
            return Some(AppError::ConfigRollbackFailed {
                restart_error: restart_error.to_string(),
                rollback_error: rollback_write_error.to_string(),
            });
        }

        let rollback_sync_config = match old_config.to_sync_config() {
            Ok(config) => config,
            Err(error) => {
                return Some(AppError::ConfigRollbackFailed {
                    restart_error: restart_error.to_string(),
                    rollback_error: error.to_string(),
                });
            }
        };

        let rollback_restart = {
            let mut runtime = self.sync_runtime.lock().await;
            runtime.restart(rollback_sync_config).await
        };

        if let Err(rollback_restart_error) = rollback_restart {
            return Some(AppError::ConfigRollbackFailed {
                restart_error: restart_error.to_string(),
                rollback_error: rollback_restart_error.to_string(),
            });
        }

        if let Err(storage_error) = self
            .storage_runtime
            .reconfigure(old_config.to_storage_config())
            .await
        {
            return Some(AppError::ConfigRollbackFailed {
                restart_error: restart_error.to_string(),
                rollback_error: storage_error.to_string(),
            });
        }

        *self.config.write().await = old_config.clone();
        None
    }
}

impl BroadcastConfig {
    fn from_config(config: &AppConfig) -> Self {
        Self {
            network_enabled: config.sync.network.enabled,
            mdns_enabled: config.sync.network.mdns_enabled,
            manual_peers: config.sync.network.manual_peers.clone(),
        }
    }
}
