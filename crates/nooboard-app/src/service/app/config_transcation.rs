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
        self.start_outbox_dispatcher_if_needed().await?;

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
        let mut rollback_errors = Vec::new();

        match old_config.to_sync_config() {
            Ok(rollback_sync_config) => {
                let rollback_restart = {
                    let mut runtime = self.sync_runtime.lock().await;
                    runtime.restart(rollback_sync_config).await
                };
                if let Err(rollback_restart_error) = rollback_restart {
                    rollback_errors.push(format!("sync rollback failed: {rollback_restart_error}"));
                } else if let Err(rollback_subscribe_error) = self
                    .subscriptions
                    .activate(Arc::clone(&self.sync_runtime))
                    .await
                {
                    rollback_errors.push(format!(
                        "subscription rollback failed: {rollback_subscribe_error}"
                    ));
                } else if let Err(dispatcher_error) = self.start_outbox_dispatcher_if_needed().await
                {
                    rollback_errors.push(format!(
                        "outbox dispatcher rollback failed: {dispatcher_error}"
                    ));
                }
            }
            Err(error) => {
                rollback_errors.push(format!("rollback sync config invalid: {error}"));
            }
        }

        if let Err(rollback_write_error) = old_config.save_atomically(&self.config_path) {
            rollback_errors.push(format!(
                "config rollback write failed: {rollback_write_error}"
            ));
        }

        if rollback_errors.is_empty() {
            *self.config.write().await = old_config.clone();
            None
        } else {
            Some(AppError::ConfigRollbackFailed {
                restart_error: restart_error.to_string(),
                rollback_error: rollback_errors.join("; "),
            })
        }
    }

    async fn rollback_storage_reconfigure_failure(
        &self,
        old_config: &AppConfig,
        apply_error: &AppError,
    ) -> Option<AppError> {
        let mut rollback_errors = Vec::new();

        if let Err(rollback_reconfigure_error) = self
            .storage_runtime
            .reconfigure(old_config.to_storage_config())
            .await
        {
            rollback_errors.push(format!(
                "storage rollback failed: {rollback_reconfigure_error}"
            ));
        }

        if let Err(rollback_write_error) = old_config.save_atomically(&self.config_path) {
            rollback_errors.push(format!(
                "config rollback write failed: {rollback_write_error}"
            ));
        }

        if rollback_errors.is_empty() {
            *self.config.write().await = old_config.clone();
            None
        } else {
            Some(AppError::ConfigRollbackFailed {
                restart_error: apply_error.to_string(),
                rollback_error: rollback_errors.join("; "),
            })
        }
    }
}
