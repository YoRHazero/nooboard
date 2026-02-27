use crate::AppResult;
use crate::config::AppConfig;

use super::{AppServiceImpl, AppSyncStatus, ConnectedPeer};

impl AppServiceImpl {
    pub(super) async fn start_engine_usecase(&self) -> AppResult<()> {
        let config = self.reload_config_from_disk().await?;
        let sync_config = config.to_sync_config()?;
        let mut runtime = self.sync_runtime.lock().await;
        runtime.start(sync_config).await
    }

    pub(super) async fn stop_engine_usecase(&self) -> AppResult<()> {
        let mut runtime = self.sync_runtime.lock().await;
        runtime.stop().await
    }

    pub(super) async fn restart_engine_usecase(&self) -> AppResult<()> {
        let _config_guard = self.config_update_lock.lock().await;
        let config = self.reload_config_from_disk().await?;
        let sync_config = config.to_sync_config()?;
        let mut runtime = self.sync_runtime.lock().await;
        runtime.restart(sync_config).await
    }

    pub(super) async fn sync_status_usecase(&self) -> AppResult<AppSyncStatus> {
        let runtime = self.sync_runtime.lock().await;
        Ok(runtime.status().into())
    }

    pub(super) async fn connected_peers_usecase(&self) -> AppResult<Vec<ConnectedPeer>> {
        let runtime = self.sync_runtime.lock().await;
        Ok(runtime
            .connected_peers()
            .into_iter()
            .map(ConnectedPeer::from)
            .collect())
    }

    pub(super) async fn reload_config_from_disk(&self) -> AppResult<AppConfig> {
        let config = AppConfig::load(&self.config_path)?;
        self.storage_runtime
            .reconfigure(config.to_storage_config())
            .await?;
        *self.config.write().await = config.clone();
        Ok(config)
    }
}
