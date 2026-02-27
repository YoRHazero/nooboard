use crate::AppError;
use crate::AppResult;
use crate::config::AppConfig;

use super::{AppServiceImpl, BroadcastConfig, NetworkPatch};

impl AppServiceImpl {
    pub(super) async fn apply_network_patch_usecase(
        &self,
        patch: NetworkPatch,
    ) -> AppResult<BroadcastConfig> {
        match patch {
            NetworkPatch::SetMdnsEnabled(enabled) => {
                self.apply_network_patch_with(|config| {
                    config.sync.network.mdns_enabled = enabled;
                    Ok(())
                })
                .await
            }
            NetworkPatch::SetNetworkEnabled(enabled) => {
                self.apply_network_patch_with(|config| {
                    config.sync.network.enabled = enabled;
                    Ok(())
                })
                .await
            }
            NetworkPatch::AddManualPeer(addr) => {
                self.apply_network_patch_with(|config| {
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
                self.apply_network_patch_with(|config| {
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

    async fn apply_network_patch_with<F>(&self, patch: F) -> AppResult<BroadcastConfig>
    where
        F: FnOnce(&mut AppConfig) -> AppResult<()>,
    {
        let applied = self.execute_network_config_transcation(patch).await?;
        Ok(BroadcastConfig::from_config(&applied))
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
