use std::sync::Arc;

use crate::config::AppConfig;
use crate::{AppError, AppResult};

use super::{AppServiceImpl, SubscriptionCloseReason};

impl AppServiceImpl {
    pub(super) async fn execute_network_config_transcation<F>(
        &self,
        patch: F,
    ) -> AppResult<AppConfig>
    where
        F: FnOnce(&mut AppConfig) -> AppResult<()>,
    {
        let _config_guard = self.config_update_lock.lock().await;

        let old_config = AppConfig::load(&self.config_path)?;
        let mut updated_config = old_config.clone();
        patch(&mut updated_config)?;
        updated_config.validate()?;

        self.persist_and_restart_sync_with_rollback(old_config, updated_config)
            .await
    }

    pub(super) async fn execute_storage_config_transcation<F>(
        &self,
        patch: F,
    ) -> AppResult<AppConfig>
    where
        F: FnOnce(&mut AppConfig) -> AppResult<()>,
    {
        let _config_guard = self.config_update_lock.lock().await;

        let old_config = AppConfig::load(&self.config_path)?;
        let mut updated_config = old_config.clone();
        patch(&mut updated_config)?;
        updated_config.validate()?;

        self.persist_and_reconfigure_storage_with_rollback(old_config, updated_config)
            .await
    }

    pub(super) async fn persist_and_restart_sync_with_rollback(
        &self,
        old_config: AppConfig,
        updated_config: AppConfig,
    ) -> AppResult<AppConfig> {
        let updated_sync_config = updated_config.to_sync_config()?;
        updated_config.save_atomically(&self.config_path)?;

        let restart_result = {
            let mut runtime = self.sync_runtime.lock().await;
            runtime.restart(updated_sync_config).await
        };

        if let Err(restart_error) = restart_result {
            self.subscriptions
                .deactivate(SubscriptionCloseReason::Fatal)
                .await;
            if let Some(rollback_failure) = self
                .rollback_sync_restart_failure(&old_config, &restart_error)
                .await
            {
                return Err(rollback_failure);
            }
            return Err(restart_error);
        }
        self.subscriptions
            .activate(Arc::clone(&self.sync_runtime))
            .await?;

        *self.config.write().await = updated_config.clone();
        Ok(updated_config)
    }

    pub(super) async fn persist_and_reconfigure_storage_with_rollback(
        &self,
        old_config: AppConfig,
        updated_config: AppConfig,
    ) -> AppResult<AppConfig> {
        updated_config.save_atomically(&self.config_path)?;

        let reconfigure_result = self
            .storage_runtime
            .reconfigure(updated_config.to_storage_config())
            .await;

        if let Err(reconfigure_error) = reconfigure_result {
            if let Some(rollback_failure) = self
                .rollback_storage_reconfigure_failure(&old_config, &reconfigure_error)
                .await
            {
                return Err(rollback_failure);
            }
            return Err(reconfigure_error);
        }

        *self.config.write().await = updated_config.clone();
        Ok(updated_config)
    }

    async fn rollback_sync_restart_failure(
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

        if let Err(rollback_subscribe_error) = self
            .subscriptions
            .activate(Arc::clone(&self.sync_runtime))
            .await
        {
            return Some(AppError::ConfigRollbackFailed {
                restart_error: restart_error.to_string(),
                rollback_error: rollback_subscribe_error.to_string(),
            });
        }

        *self.config.write().await = old_config.clone();
        None
    }

    async fn rollback_storage_reconfigure_failure(
        &self,
        old_config: &AppConfig,
        apply_error: &AppError,
    ) -> Option<AppError> {
        if let Err(rollback_write_error) = old_config.save_atomically(&self.config_path) {
            return Some(AppError::ConfigRollbackFailed {
                restart_error: apply_error.to_string(),
                rollback_error: rollback_write_error.to_string(),
            });
        }

        if let Err(rollback_reconfigure_error) = self
            .storage_runtime
            .reconfigure(old_config.to_storage_config())
            .await
        {
            return Some(AppError::ConfigRollbackFailed {
                restart_error: apply_error.to_string(),
                rollback_error: rollback_reconfigure_error.to_string(),
            });
        }

        *self.config.write().await = old_config.clone();
        None
    }
}
