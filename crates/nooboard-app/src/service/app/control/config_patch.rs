use crate::service::types::{
    ClipboardSettingsPatch, ConnectionIdentitySettingsPatch, NetworkSettingsPatch, SettingsPatch,
    StorageSettingsPatch, TransferSettingsPatch,
};
use crate::{AppError, AppResult};
use nooboard_config::AppConfig;

use super::engine_reconcile::reconcile_engine_state;
use super::state::ControlState;

#[derive(Default)]
struct PatchEffect {
    storage_reconfigure_required: bool,
    sync_reconcile_required: bool,
    local_capture_reconcile_required: bool,
}

pub(super) async fn patch_settings(
    state: &mut ControlState,
    patch: SettingsPatch,
) -> AppResult<()> {
    let old_config = state.config.clone();
    let mut updated_config = old_config.clone();
    let effect = apply_patch_to_config(state, &mut updated_config, patch);
    updated_config.validate()?;
    updated_config.save_atomically(&state.config_path)?;

    let mut apply_error = None;

    if effect.storage_reconfigure_required
        && let Err(error) = state
            .storage_runtime
            .reconfigure(updated_config.to_storage_config())
            .await
    {
        apply_error = Some(error);
    }

    if apply_error.is_none() {
        state.config = updated_config;
        let identity = state.identity_state();
        let local_connection = state.local_connection_state();
        let settings = state.settings_state();
        state.update_state(|app_state| {
            app_state.identity = identity;
            app_state.local_connection = local_connection;
            app_state.settings = settings;
        });

        if effect.local_capture_reconcile_required
            && let Err(error) = reconcile_local_capture_runtime(state).await
        {
            apply_error = Some(error);
        }
    }

    if apply_error.is_none()
        && effect.sync_reconcile_required
        && let Err(error) = reconcile_engine_state(state, true).await
    {
        apply_error = Some(error);
    }

    if let Some(error) = apply_error {
        if let Some(rollback_error) =
            rollback_patch_failure(state, &old_config, &error, &effect).await
        {
            return Err(rollback_error);
        }
        return Err(error);
    }

    Ok(())
}

pub(super) async fn reconcile_local_capture_runtime(state: &mut ControlState) -> AppResult<()> {
    if state.config.local_capture_enabled() {
        state.clipboard.start_watch()?;
    } else {
        state.clipboard.stop_watch().await?;
    }
    Ok(())
}

fn apply_patch_to_config(
    state: &ControlState,
    config: &mut AppConfig,
    patch: SettingsPatch,
) -> PatchEffect {
    match patch {
        SettingsPatch::ConnectionIdentity(patch) => apply_connection_identity_patch(config, patch),
        SettingsPatch::Network(patch) => apply_network_patch(config, patch),
        SettingsPatch::Storage(patch) => {
            apply_storage_patch(config, state.config_base_dir(), patch)
        }
        SettingsPatch::Clipboard(patch) => apply_clipboard_patch(config, patch),
        SettingsPatch::Transfers(patch) => {
            apply_transfer_patch(config, state.config_base_dir(), patch)
        }
    }
}

fn apply_connection_identity_patch(
    config: &mut AppConfig,
    patch: ConnectionIdentitySettingsPatch,
) -> PatchEffect {
    match patch {
        ConnectionIdentitySettingsPatch::Replace(connection_identity) => {
            config.identity.device_id = connection_identity.device_id;
            config.sync.auth.token = connection_identity.token;
        }
    }

    PatchEffect {
        sync_reconcile_required: true,
        ..PatchEffect::default()
    }
}

fn apply_network_patch(config: &mut AppConfig, patch: NetworkSettingsPatch) -> PatchEffect {
    match patch {
        NetworkSettingsPatch::SetListenPort(port) => {
            config.sync.network.listen_addr.set_port(port);
        }
        NetworkSettingsPatch::SetNetworkEnabled(enabled) => {
            config.sync.network.enabled = enabled;
        }
        NetworkSettingsPatch::SetMdnsEnabled(enabled) => {
            config.sync.network.mdns_enabled = enabled;
        }
        NetworkSettingsPatch::SetManualPeers(peers) => {
            config.sync.network.manual_peers = peers;
        }
    }

    PatchEffect {
        sync_reconcile_required: true,
        ..PatchEffect::default()
    }
}

fn apply_storage_patch(
    config: &mut AppConfig,
    config_base_dir: std::path::PathBuf,
    patch: StorageSettingsPatch,
) -> PatchEffect {
    if let Some(db_root) = patch.db_root {
        config.storage.db_root = if db_root.is_relative() {
            config_base_dir.join(db_root)
        } else {
            db_root
        };
    }
    if let Some(history_window_days) = patch.history_window_days {
        config.storage.lifecycle.history_window_days = history_window_days;
    }
    if let Some(dedup_window_days) = patch.dedup_window_days {
        config.storage.lifecycle.dedup_window_days = dedup_window_days;
    }
    if let Some(max_text_bytes) = patch.max_text_bytes {
        config.storage.max_text_bytes = max_text_bytes;
    }
    if let Some(gc_batch_size) = patch.gc_batch_size {
        config.storage.lifecycle.gc_batch_size = u32::try_from(gc_batch_size).unwrap_or(u32::MAX);
    }

    PatchEffect {
        storage_reconfigure_required: true,
        ..PatchEffect::default()
    }
}

fn apply_clipboard_patch(config: &mut AppConfig, patch: ClipboardSettingsPatch) -> PatchEffect {
    match patch {
        ClipboardSettingsPatch::SetLocalCaptureEnabled(enabled) => {
            config.app.clipboard.local_capture_enabled = enabled;
        }
    }

    PatchEffect {
        local_capture_reconcile_required: true,
        ..PatchEffect::default()
    }
}

fn apply_transfer_patch(
    config: &mut AppConfig,
    config_base_dir: std::path::PathBuf,
    patch: TransferSettingsPatch,
) -> PatchEffect {
    match patch {
        TransferSettingsPatch::SetDownloadDir(download_dir) => {
            config.sync.file.download_dir = if download_dir.is_relative() {
                config_base_dir.join(download_dir)
            } else {
                download_dir
            };
        }
    }

    PatchEffect {
        sync_reconcile_required: true,
        ..PatchEffect::default()
    }
}

async fn rollback_patch_failure(
    state: &mut ControlState,
    old_config: &AppConfig,
    apply_error: &AppError,
    effect: &PatchEffect,
) -> Option<AppError> {
    let mut rollback_errors = Vec::new();

    if effect.storage_reconfigure_required
        && let Err(error) = state
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
    let identity = state.identity_state();
    let local_connection = state.local_connection_state();
    let settings = state.settings_state();
    state.update_state(|app_state| {
        app_state.identity = identity;
        app_state.local_connection = local_connection;
        app_state.settings = settings;
    });

    if effect.local_capture_reconcile_required
        && let Err(error) = reconcile_local_capture_runtime(state).await
    {
        rollback_errors.push(format!("clipboard rollback failed: {error}"));
    }

    if effect.sync_reconcile_required
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
