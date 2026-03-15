use crate::error::{CoreError, CoreResult};
use crate::types::WorkspaceEvent;
use crate::{
    ConnectDirectOutcome, DirectRequestId, DirectSeedId, DirectSeedInfo, PendingDirectRequest,
    SessionId, SessionInfo,
};

use super::super::state::WorkspaceState;
use super::clipboard;
use super::snapshot::{finish_with_snapshot_refresh, refresh_snapshot_from_runtime};

pub(crate) async fn start_network(state: &mut WorkspaceState) -> CoreResult<()> {
    let result = state.network_runtime().start().await.map_err(Into::into);
    finish_with_snapshot_refresh(state, result).await
}

pub(crate) async fn stop_network(state: &mut WorkspaceState) -> CoreResult<()> {
    let result = state.network_runtime().shutdown().await.map_err(Into::into);
    finish_with_snapshot_refresh(state, result).await
}

pub(crate) async fn list_direct_seeds(state: &WorkspaceState) -> CoreResult<Vec<DirectSeedInfo>> {
    state
        .network_runtime()
        .list_direct_seeds()
        .await
        .map_err(Into::into)
}

pub(crate) async fn search_direct_seeds(
    state: &WorkspaceState,
    query: &str,
) -> CoreResult<Vec<DirectSeedInfo>> {
    state
        .network_runtime()
        .search_direct_seeds(query)
        .await
        .map_err(Into::into)
}

pub(crate) async fn connect_direct_seed(
    state: &mut WorkspaceState,
    id: DirectSeedId,
) -> CoreResult<ConnectDirectOutcome> {
    let result = state
        .network_runtime()
        .connect_direct_seed(id)
        .await
        .map_err(Into::into);
    finish_with_snapshot_refresh(state, result).await
}

pub(crate) async fn list_pending_direct_requests(
    state: &WorkspaceState,
) -> CoreResult<Vec<PendingDirectRequest>> {
    state
        .network_runtime()
        .list_pending_direct_requests()
        .await
        .map_err(Into::into)
}

pub(crate) async fn approve_direct_request(
    state: &mut WorkspaceState,
    id: DirectRequestId,
) -> CoreResult<()> {
    let result = state
        .network_runtime()
        .approve_direct_request(id)
        .await
        .map_err(Into::into);
    finish_with_snapshot_refresh(state, result).await
}

pub(crate) async fn reject_direct_request(
    state: &mut WorkspaceState,
    id: DirectRequestId,
) -> CoreResult<()> {
    let result = state
        .network_runtime()
        .reject_direct_request(id)
        .await
        .map_err(Into::into);
    finish_with_snapshot_refresh(state, result).await
}

pub(crate) async fn list_sessions(state: &WorkspaceState) -> CoreResult<Vec<SessionInfo>> {
    state
        .network_runtime()
        .list_sessions()
        .await
        .map_err(Into::into)
}

pub(crate) async fn disconnect_session(
    state: &mut WorkspaceState,
    id: SessionId,
) -> CoreResult<()> {
    let result = state
        .network_runtime()
        .disconnect_session(id)
        .await
        .map_err(map_disconnect_session_error);
    finish_with_snapshot_refresh(state, result).await
}

pub(crate) async fn handle_network_event(
    state: &mut WorkspaceState,
    event: nooboard_network::NetworkEvent,
) -> CoreResult<()> {
    match event {
        nooboard_network::NetworkEvent::StatusChanged(_)
        | nooboard_network::NetworkEvent::LanPeersChanged
        | nooboard_network::NetworkEvent::DirectSeedsChanged
        | nooboard_network::NetworkEvent::PendingDirectRequestsChanged
        | nooboard_network::NetworkEvent::SessionsChanged => {
            refresh_snapshot_from_runtime(state).await?;
        }
        nooboard_network::NetworkEvent::ConnectionFailed(failure) => {
            refresh_snapshot_from_runtime(state).await?;
            state.publish_event(WorkspaceEvent::NetworkConnectionFailed { failure });
        }
        nooboard_network::NetworkEvent::TextReceived {
            event_id,
            content,
            peer_noob_id,
            peer_device_id,
            ..
        } => {
            clipboard::handle_remote_text_received(
                state,
                &event_id,
                content,
                peer_noob_id,
                peer_device_id,
            )
            .await?;
        }
        nooboard_network::NetworkEvent::IncomingTransferOffered { offer } => {
            refresh_snapshot_from_runtime(state).await?;
            state.publish_event(WorkspaceEvent::IncomingTransferOffered {
                ticket: offer.ticket,
            });
        }
        nooboard_network::NetworkEvent::TransferUpdated { transfer } => {
            refresh_snapshot_from_runtime(state).await?;
            state.publish_event(WorkspaceEvent::TransferUpdated {
                ticket: transfer.ticket,
            });
        }
        nooboard_network::NetworkEvent::TransferCompleted { transfer } => {
            refresh_snapshot_from_runtime(state).await?;
            state.publish_event(WorkspaceEvent::TransferCompleted {
                ticket: transfer.ticket,
                outcome: transfer.outcome,
            });
        }
    }

    Ok(())
}

fn map_disconnect_session_error(error: nooboard_network::NetworkError) -> CoreError {
    match error {
        nooboard_network::NetworkError::SessionNotFound(session_id) => CoreError::SessionNotFound {
            session_id: session_id.to_string(),
        },
        other => other.into(),
    }
}

#[cfg(test)]
mod tests {
    use crate::CoreError;

    use super::map_disconnect_session_error;

    #[test]
    fn maps_runtime_session_not_found_to_core_session_not_found() {
        let session_id = crate::SessionId::new();
        let error = map_disconnect_session_error(nooboard_network::NetworkError::SessionNotFound(
            session_id,
        ));

        assert!(matches!(
            error,
            CoreError::SessionNotFound { session_id: value } if value == session_id.to_string()
        ));
    }
}
