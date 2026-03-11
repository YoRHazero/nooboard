use crate::service::types::SyncDesiredState;
use crate::{AppError, AppResult};

use super::state::ControlState;

pub(super) async fn set_sync_desired_state(
    state: &mut ControlState,
    desired_state: SyncDesiredState,
) -> AppResult<()> {
    if matches!(desired_state, SyncDesiredState::Running) && !state.config.sync.network.enabled {
        return Err(AppError::SyncDisabled);
    }

    state.update_state(|app_state| {
        app_state.sync.desired = desired_state;
    });
    reconcile_engine_state(state, false).await
}

pub(super) async fn reconcile_engine_state(
    state: &mut ControlState,
    force_restart: bool,
) -> AppResult<()> {
    if !state.config.sync.network.enabled {
        state.sync_runtime.stop().await?;
        state.sync_runtime.mark_disabled();

        let actual = state.sync_actual_status();
        let local_connection = state.local_connection_state();
        let peers = state.connected_peers_state();
        state.update_state(|app_state| {
            app_state.sync.desired = SyncDesiredState::Stopped;
            app_state.sync.actual = actual;
            app_state.local_connection = local_connection;
            app_state.peers.connected = peers;
        });
        return Ok(());
    }

    match state.app_state.sync.desired {
        SyncDesiredState::Running => {
            let has_engine = state.sync_runtime.has_engine();
            let should_reload = force_restart || !has_engine;
            if should_reload {
                let sync_config = state.config.to_sync_config()?;
                if has_engine {
                    state.sync_runtime.restart(sync_config).await?;
                } else {
                    state.sync_runtime.start(sync_config).await?;
                }
            }
        }
        SyncDesiredState::Stopped => {
            state.sync_runtime.stop().await?;
        }
    }

    let actual = state.sync_actual_status();
    let local_connection = state.local_connection_state();
    let peers = state.connected_peers_state();
    state.update_state(|app_state| {
        app_state.sync.actual = actual;
        app_state.local_connection = local_connection;
        app_state.peers.connected = peers;
    });
    Ok(())
}

pub(super) async fn shutdown(state: &mut ControlState) -> AppResult<()> {
    state.update_state(|app_state| {
        app_state.sync.desired = SyncDesiredState::Stopped;
    });

    let clipboard_result = state.clipboard.stop_watch().await;
    let stop_result = state.sync_runtime.stop().await;
    let storage_result = state.storage_runtime.shutdown().await;
    let actual = state.sync_actual_status();

    state.update_state(|app_state| {
        app_state.sync.actual = actual;
        app_state.peers.connected.clear();
    });

    if let Err(error) = clipboard_result {
        return Err(error);
    }
    if let Err(error) = stop_result {
        return Err(error);
    }
    storage_result
}
