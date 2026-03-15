use std::path::{Path, PathBuf};

use nooboard_config::{AppConfig, DirectSeedConfig};
use nooboard_network::{DirectSeedId, NetworkRuntime, UpsertDirectSeedInput};

use crate::error::{CoreError, CoreResult};
use crate::workspace::local_connection::detect_device_endpoint;

use super::super::state::WorkspaceState;
use super::snapshot::finish_with_snapshot_refresh;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ConfigMutation {
    pub(crate) restart_network_bridge: bool,
}

pub(crate) struct ConfigOutcome<T> {
    pub(crate) post_action: ConfigMutation,
    pub(crate) result: CoreResult<T>,
}

impl<T> ConfigOutcome<T> {
    fn new(post_action: ConfigMutation, result: CoreResult<T>) -> Self {
        Self {
            post_action,
            result,
        }
    }
}

pub(crate) async fn set_device_id(state: &mut WorkspaceState, value: String) -> ConfigOutcome<()> {
    let mut updated = state.config().clone();
    updated.identity.device_id = value;

    let network_config = match validate_and_build_network(&updated) {
        Ok(config) => config,
        Err(error) => return ConfigOutcome::new(ConfigMutation::default(), Err(error)),
    };

    match persist_and_apply_config(state, updated) {
        Ok(()) => {}
        Err(error) => return ConfigOutcome::new(ConfigMutation::default(), Err(error)),
    }

    let (post_action, side_effect) = rebuild_network_runtime(state, network_config).await;
    let result = finish_with_snapshot_refresh(state, side_effect).await;
    ConfigOutcome::new(post_action, result)
}

pub(crate) async fn set_network_token(
    state: &mut WorkspaceState,
    value: String,
) -> ConfigOutcome<()> {
    let mut updated = state.config().clone();
    updated.network.auth.token = value;

    let network_config = match validate_and_build_network(&updated) {
        Ok(config) => config,
        Err(error) => return ConfigOutcome::new(ConfigMutation::default(), Err(error)),
    };

    match persist_and_apply_config(state, updated) {
        Ok(()) => {}
        Err(error) => return ConfigOutcome::new(ConfigMutation::default(), Err(error)),
    }

    let (post_action, side_effect) = rebuild_network_runtime(state, network_config).await;
    let result = finish_with_snapshot_refresh(state, side_effect).await;
    ConfigOutcome::new(post_action, result)
}

pub(crate) async fn set_network_listen_port(
    state: &mut WorkspaceState,
    value: u16,
) -> ConfigOutcome<()> {
    let mut updated = state.config().clone();
    updated.network.listen_port = value;

    let network_config = match validate_and_build_network(&updated) {
        Ok(config) => config,
        Err(error) => return ConfigOutcome::new(ConfigMutation::default(), Err(error)),
    };

    match persist_and_apply_config(state, updated) {
        Ok(()) => {}
        Err(error) => return ConfigOutcome::new(ConfigMutation::default(), Err(error)),
    }

    let (post_action, side_effect) = rebuild_network_runtime(state, network_config).await;
    let result = finish_with_snapshot_refresh(state, side_effect).await;
    ConfigOutcome::new(post_action, result)
}

pub(crate) async fn set_lan_enabled(state: &mut WorkspaceState, value: bool) -> ConfigOutcome<()> {
    let mut updated = state.config().clone();
    updated.network.lan.enabled = value;

    if let Err(error) = validate_config(&updated) {
        return ConfigOutcome::new(ConfigMutation::default(), Err(error));
    }
    if let Err(error) = persist_and_apply_config(state, updated) {
        return ConfigOutcome::new(ConfigMutation::default(), Err(error));
    }

    let side_effect = state
        .network_runtime()
        .set_lan_enabled(value)
        .await
        .map_err(Into::into);
    let result = finish_with_snapshot_refresh(state, side_effect).await;
    ConfigOutcome::new(ConfigMutation::default(), result)
}

pub(crate) async fn set_local_capture_enabled(
    state: &mut WorkspaceState,
    value: bool,
) -> ConfigOutcome<()> {
    let mut updated = state.config().clone();
    updated.app.clipboard.local_capture_enabled = value;

    if let Err(error) = validate_config(&updated) {
        return ConfigOutcome::new(ConfigMutation::default(), Err(error));
    }
    if let Err(error) = persist_and_apply_config(state, updated) {
        return ConfigOutcome::new(ConfigMutation::default(), Err(error));
    }

    let side_effect = if value {
        state.clipboard_runtime().start_watch()
    } else {
        state.clipboard_runtime().stop_watch().await
    };
    let result = finish_with_snapshot_refresh(state, side_effect).await;
    ConfigOutcome::new(ConfigMutation::default(), result)
}

pub(crate) async fn set_download_dir(
    state: &mut WorkspaceState,
    value: PathBuf,
) -> ConfigOutcome<()> {
    let mut updated = state.config().clone();
    updated.network.transfer.download_dir = normalize_path(value, &state.config_base_dir());

    let network_config = match validate_and_build_network(&updated) {
        Ok(config) => config,
        Err(error) => return ConfigOutcome::new(ConfigMutation::default(), Err(error)),
    };

    match persist_and_apply_config(state, updated) {
        Ok(()) => {}
        Err(error) => return ConfigOutcome::new(ConfigMutation::default(), Err(error)),
    }

    let (post_action, side_effect) = rebuild_network_runtime(state, network_config).await;
    let result = finish_with_snapshot_refresh(state, side_effect).await;
    ConfigOutcome::new(post_action, result)
}

pub(crate) async fn set_storage_settings(
    state: &mut WorkspaceState,
    input: crate::StorageSettingsInput,
) -> ConfigOutcome<()> {
    let mut updated = state.config().clone();
    updated.storage.lifecycle.history_window_days = input.history_window_days;
    updated.storage.lifecycle.dedup_window_days = input.dedup_window_days;
    updated.storage.max_text_bytes = input.max_text_bytes;
    updated.storage.lifecycle.gc_batch_size = match u32::try_from(input.gc_batch_size) {
        Ok(value) => value,
        Err(_) => {
            return ConfigOutcome::new(
                ConfigMutation::default(),
                Err(CoreError::InvalidState(format!(
                    "gc_batch_size is out of range for u32: {}",
                    input.gc_batch_size
                ))),
            );
        }
    };

    if let Err(error) = validate_config(&updated) {
        return ConfigOutcome::new(ConfigMutation::default(), Err(error));
    }
    let storage_config = updated.to_storage_config();
    if let Err(error) = storage_config.validate() {
        return ConfigOutcome::new(ConfigMutation::default(), Err(error.into()));
    }
    if let Err(error) = persist_and_apply_config(state, updated) {
        return ConfigOutcome::new(ConfigMutation::default(), Err(error));
    }

    let side_effect = state.storage_runtime().reconfigure(storage_config).await;
    let result = finish_with_snapshot_refresh(state, side_effect).await;
    ConfigOutcome::new(ConfigMutation::default(), result)
}

pub(crate) async fn upsert_direct_seed(
    state: &mut WorkspaceState,
    input: UpsertDirectSeedInput,
) -> ConfigOutcome<DirectSeedId> {
    let mut updated = state.config().clone();
    let runtime_input = apply_upsert_direct_seed(&mut updated, input);

    if let Err(error) = validate_config(&updated) {
        return ConfigOutcome::new(ConfigMutation::default(), Err(error));
    }
    if let Err(error) = persist_and_apply_config(state, updated) {
        return ConfigOutcome::new(ConfigMutation::default(), Err(error));
    }

    let side_effect = state
        .network_runtime()
        .upsert_direct_seed(runtime_input)
        .await
        .map_err(Into::into);
    let result = finish_with_snapshot_refresh(state, side_effect).await;
    ConfigOutcome::new(ConfigMutation::default(), result)
}

pub(crate) async fn remove_direct_seed(
    state: &mut WorkspaceState,
    id: DirectSeedId,
) -> ConfigOutcome<()> {
    let mut updated = state.config().clone();
    if !remove_direct_seed_from_config(&mut updated, id) {
        return ConfigOutcome::new(
            ConfigMutation::default(),
            Err(nooboard_network::NetworkError::DirectSeedNotFound(id).into()),
        );
    }

    if let Err(error) = validate_config(&updated) {
        return ConfigOutcome::new(ConfigMutation::default(), Err(error));
    }
    if let Err(error) = persist_and_apply_config(state, updated) {
        return ConfigOutcome::new(ConfigMutation::default(), Err(error));
    }

    let side_effect = state
        .network_runtime()
        .remove_direct_seed(id)
        .await
        .map_err(Into::into);
    let result = finish_with_snapshot_refresh(state, side_effect).await;
    ConfigOutcome::new(ConfigMutation::default(), result)
}

fn validate_config(config: &AppConfig) -> CoreResult<()> {
    config.validate()?;
    let storage_config = config.to_storage_config();
    storage_config.validate()?;
    Ok(())
}

fn validate_and_build_network(config: &AppConfig) -> CoreResult<nooboard_network::NetworkConfig> {
    validate_config(config)?;
    config.to_network_config().map_err(Into::into)
}

fn persist_and_apply_config(state: &mut WorkspaceState, updated: AppConfig) -> CoreResult<()> {
    updated.save_atomically(state.config_path())?;
    state.replace_config(updated);
    state.set_local_connection(crate::LocalConnectionInfo {
        device_endpoint: detect_device_endpoint(state.config().network.listen_port),
    });
    Ok(())
}

async fn rebuild_network_runtime(
    state: &mut WorkspaceState,
    network_config: nooboard_network::NetworkConfig,
) -> (ConfigMutation, CoreResult<()>) {
    let was_running = matches!(
        state.current_snapshot().network.status,
        crate::NetworkStatus::Running | crate::NetworkStatus::Starting
    );
    let old_runtime = state.network_runtime().clone();
    let new_runtime = match NetworkRuntime::new(network_config) {
        Ok(runtime) => runtime,
        Err(error) => return (ConfigMutation::default(), Err(error.into())),
    };

    if was_running && let Err(error) = old_runtime.shutdown().await {
        return (ConfigMutation::default(), Err(error.into()));
    }

    let post_action = ConfigMutation {
        restart_network_bridge: true,
    };
    let start_result = if was_running {
        new_runtime.start().await.map_err(Into::into)
    } else {
        Ok(())
    };
    state.replace_network_runtime(new_runtime);
    (post_action, start_result)
}

fn apply_upsert_direct_seed(
    config: &mut AppConfig,
    input: UpsertDirectSeedInput,
) -> UpsertDirectSeedInput {
    let id = input.id.unwrap_or_else(DirectSeedId::new);
    let replacement = DirectSeedConfig {
        id: id.as_uuid(),
        label: input.label.clone(),
        host: input.host.clone(),
        port: input.port,
        enabled: input.enabled,
    };

    if let Some(existing) = config
        .network
        .direct
        .seeds
        .iter_mut()
        .find(|seed| seed.id == id.as_uuid())
    {
        *existing = replacement;
    } else {
        config.network.direct.seeds.push(replacement);
    }

    UpsertDirectSeedInput {
        id: Some(id),
        label: input.label,
        host: input.host,
        port: input.port,
        enabled: input.enabled,
    }
}

fn remove_direct_seed_from_config(config: &mut AppConfig, id: DirectSeedId) -> bool {
    let Some(position) = config
        .network
        .direct
        .seeds
        .iter()
        .position(|seed| seed.id == id.as_uuid())
    else {
        return false;
    };
    config.network.direct.seeds.remove(position);
    true
}

fn normalize_path(path: PathBuf, base_dir: &Path) -> PathBuf {
    if path.is_relative() {
        base_dir.join(path)
    } else {
        path
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::normalize_path;

    #[test]
    fn normalize_relative_path_against_config_parent() {
        let normalized = normalize_path(
            Path::new("downloads").to_path_buf(),
            Path::new("/tmp/config-root"),
        );
        assert_eq!(normalized, Path::new("/tmp/config-root").join("downloads"));
    }
}
