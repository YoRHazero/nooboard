use crate::AppResult;
use crate::service::types::{AppServiceSnapshot, SubscriptionCloseReason, SyncDesiredState};

use super::state::ControlState;

pub(super) async fn set_sync_desired_state(
    state: &mut ControlState,
    desired_state: SyncDesiredState,
) -> AppResult<AppServiceSnapshot> {
    state.desired_state = desired_state;
    reconcile_engine_state(state, false).await?;
    Ok(state.snapshot())
}

pub(super) async fn reconcile_engine_state(
    state: &mut ControlState,
    force_restart: bool,
) -> AppResult<()> {
    match state.desired_state {
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
                state.subscriptions.activate(&state.sync_runtime).await?;
            }
        }
        SyncDesiredState::Stopped => {
            state
                .subscriptions
                .deactivate(SubscriptionCloseReason::EngineStopped)
                .await;
            state.sync_runtime.stop().await?;
        }
    }

    Ok(())
}

pub(super) async fn shutdown(state: &mut ControlState) -> AppResult<()> {
    state.desired_state = SyncDesiredState::Stopped;
    state
        .subscriptions
        .deactivate(SubscriptionCloseReason::EngineStopped)
        .await;
    let stop_result = state.sync_runtime.stop().await;
    let storage_result = state.storage_runtime.shutdown().await;

    if let Err(error) = stop_result {
        return Err(error);
    }
    storage_result
}
