use std::sync::Arc;

use crate::config::AppConfig;
use crate::{AppError, AppResult};

use super::{AppServiceImpl, AppSyncStatus, ConnectedPeer, SubscriptionCloseReason};

impl AppServiceImpl {
    pub(super) async fn start_engine_usecase(&self) -> AppResult<()> {
        let config = self.reload_config_from_disk().await?;
        let sync_config = config.to_sync_config()?;
        {
            let mut runtime = self.sync_runtime.lock().await;
            runtime.start(sync_config).await?;
        }
        self.subscriptions
            .activate(Arc::clone(&self.sync_runtime))
            .await
    }

    pub(super) async fn stop_engine_usecase(&self) -> AppResult<()> {
        self.subscriptions
            .deactivate(SubscriptionCloseReason::EngineStopped)
            .await;
        let mut runtime = self.sync_runtime.lock().await;
        runtime.stop().await
    }

    pub(super) async fn restart_engine_usecase(&self) -> AppResult<()> {
        let _config_guard = self.config_update_lock.lock().await;
        let old_config = self.config.read().await.clone();
        let updated_config = AppConfig::load(&self.config_path)?;

        let old_storage_config = old_config.to_storage_config();
        let old_sync_config = old_config.to_sync_config()?;
        let updated_storage_config = updated_config.to_storage_config();
        let updated_sync_config = updated_config.to_sync_config()?;

        self.storage_runtime
            .reconfigure(updated_storage_config)
            .await?;

        let restart_result = {
            let mut runtime = self.sync_runtime.lock().await;
            runtime.restart(updated_sync_config).await
        };

        if let Err(restart_error) = restart_result {
            self.subscriptions
                .deactivate(SubscriptionCloseReason::Fatal)
                .await;
            if let Some(rollback_failure) = self
                .rollback_restart_failure(old_storage_config, old_sync_config, &restart_error)
                .await
            {
                return Err(rollback_failure);
            }
            return Err(restart_error);
        }
        self.subscriptions
            .activate(Arc::clone(&self.sync_runtime))
            .await?;

        *self.config.write().await = updated_config;
        Ok(())
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

    async fn rollback_restart_failure(
        &self,
        old_storage_config: nooboard_storage::AppConfig,
        old_sync_config: nooboard_sync::SyncConfig,
        restart_error: &AppError,
    ) -> Option<AppError> {
        let mut rollback_errors = Vec::new();

        if let Err(storage_error) = self.storage_runtime.reconfigure(old_storage_config).await {
            rollback_errors.push(format!("storage rollback failed: {storage_error}"));
        }

        let sync_rollback = {
            let mut runtime = self.sync_runtime.lock().await;
            runtime.restart(old_sync_config).await
        };
        if let Err(sync_error) = sync_rollback {
            rollback_errors.push(format!("sync rollback failed: {sync_error}"));
        } else if let Err(subscribe_error) = self
            .subscriptions
            .activate(Arc::clone(&self.sync_runtime))
            .await
        {
            rollback_errors.push(format!("subscription rollback failed: {subscribe_error}"));
        }

        if rollback_errors.is_empty() {
            None
        } else {
            Some(AppError::ConfigRollbackFailed {
                restart_error: restart_error.to_string(),
                rollback_error: rollback_errors.join("; "),
            })
        }
    }
}
