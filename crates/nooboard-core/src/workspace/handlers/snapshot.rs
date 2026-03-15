use crate::error::CoreResult;

use super::super::state::WorkspaceState;

pub(crate) async fn refresh_snapshot_from_runtime(state: &mut WorkspaceState) -> CoreResult<()> {
    let network_snapshot = state.network_runtime().snapshot();
    state.refresh_snapshot(network_snapshot);
    Ok(())
}

pub(crate) async fn finish_with_snapshot_refresh<T>(
    state: &mut WorkspaceState,
    result: CoreResult<T>,
) -> CoreResult<T> {
    let refresh_result = refresh_snapshot_from_runtime(state).await;
    match result {
        Ok(value) => {
            refresh_result?;
            Ok(value)
        }
        Err(error) => {
            if let Err(refresh_error) = refresh_result {
                tracing::warn!(
                    "workspace snapshot refresh failed after operation error: {refresh_error}"
                );
            }
            Err(error)
        }
    }
}
