use crate::config::AppConfig;
use crate::service::types::{
    AppPatch, AppServiceSnapshot, NetworkPatch, StoragePatch, SyncDesiredState,
};
use crate::{AppError, AppResult};

use super::engine_reconcile::reconcile_engine_state;
use super::state::ControlState;

pub(super) async fn apply_config_patch(
    state: &mut ControlState,
    patch: AppPatch,
) -> AppResult<AppServiceSnapshot> {
    let old_config = state.config.clone();
    let mut updated_config = old_config.clone();
    let sync_reconcile_required = apply_patch_to_config(state, &mut updated_config, patch)?;
    updated_config.validate()?;
    updated_config.save_atomically(&state.config_path)?;

    let mut apply_error = None;

    if let Err(error) = state
        .storage_runtime
        .reconfigure(updated_config.to_storage_config())
        .await
    {
        apply_error = Some(error);
    }

    if apply_error.is_none() {
        state.config = updated_config;
        if let Err(error) = reconcile_engine_state(state, sync_reconcile_required).await {
            state
                .subscriptions
                .deactivate(crate::service::types::SubscriptionCloseReason::Fatal)
                .await;
            apply_error = Some(error);
        }
    }

    if let Some(error) = apply_error {
        if let Some(rollback_error) =
            rollback_patch_failure(state, &old_config, &error, sync_reconcile_required).await
        {
            return Err(rollback_error);
        }
        return Err(error);
    }

    Ok(state.snapshot())
}

fn apply_patch_to_config(
    state: &ControlState,
    config: &mut AppConfig,
    patch: AppPatch,
) -> AppResult<bool> {
    match patch {
        AppPatch::Network(patch) => {
            apply_network_patch(config, patch)?;
            Ok(true)
        }
        AppPatch::Storage(patch) => {
            apply_storage_patch(config, state.config_base_dir(), patch);
            Ok(false)
        }
    }
}

fn apply_network_patch(config: &mut AppConfig, patch: NetworkPatch) -> AppResult<()> {
    match patch {
        NetworkPatch::SetMdnsEnabled(enabled) => {
            config.sync.network.mdns_enabled = enabled;
        }
        NetworkPatch::SetNetworkEnabled(enabled) => {
            config.sync.network.enabled = enabled;
        }
        NetworkPatch::AddManualPeer(addr) => {
            if config.sync.network.manual_peers.contains(&addr) {
                return Err(AppError::ManualPeerExists {
                    peer: addr.to_string(),
                });
            }
            config.sync.network.manual_peers.push(addr);
        }
        NetworkPatch::RemoveManualPeer(addr) => {
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
        }
    }

    Ok(())
}

fn apply_storage_patch(
    config: &mut AppConfig,
    config_base_dir: std::path::PathBuf,
    patch: StoragePatch,
) {
    let StoragePatch {
        db_root,
        retain_old_versions,
        history_window_days,
        dedup_window_days,
        gc_every_inserts,
        gc_batch_size,
    } = patch;

    if let Some(db_root) = db_root {
        config.storage.db_root = if db_root.is_relative() {
            config_base_dir.join(db_root)
        } else {
            db_root
        };
    }
    if let Some(retain_old_versions) = retain_old_versions {
        config.storage.retain_old_versions = retain_old_versions;
    }
    if let Some(history_window_days) = history_window_days {
        config.storage.lifecycle.history_window_days = history_window_days;
    }
    if let Some(dedup_window_days) = dedup_window_days {
        config.storage.lifecycle.dedup_window_days = dedup_window_days;
    }
    if let Some(gc_every_inserts) = gc_every_inserts {
        config.storage.lifecycle.gc_every_inserts = gc_every_inserts;
    }
    if let Some(gc_batch_size) = gc_batch_size {
        config.storage.lifecycle.gc_batch_size = gc_batch_size;
    }
}

async fn rollback_patch_failure(
    state: &mut ControlState,
    old_config: &AppConfig,
    apply_error: &AppError,
    sync_reconcile_required: bool,
) -> Option<AppError> {
    let mut rollback_errors = Vec::new();

    if let Err(error) = state
        .storage_runtime
        .reconfigure(old_config.to_storage_config())
        .await
    {
        rollback_errors.push(format!("storage rollback failed: {error}"));
    }

    if let Err(error) = old_config.save_atomically(&state.config_path) {
        rollback_errors.push(format!("config rollback write failed: {error}"));
    }

    state.config = old_config.clone();

    if sync_reconcile_required
        && matches!(state.desired_state, SyncDesiredState::Running)
        && let Err(error) = reconcile_engine_state(state, true).await
    {
        rollback_errors.push(format!("sync rollback failed: {error}"));
    }

    if rollback_errors.is_empty() {
        None
    } else {
        Some(AppError::ConfigRollbackFailed {
            restart_error: apply_error.to_string(),
            rollback_error: rollback_errors.join("; "),
        })
    }
}
