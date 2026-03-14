use crate::error::{CoreError, CoreResult};
use crate::types::WorkspaceEvent;
use crate::{
    ConnectDirectOutcome, DirectRequestId, DirectSeedId, DirectSeedInfo, PendingDirectRequest,
    SessionId, SessionInfo,
};

use super::super::state::WorkspaceState;
use super::clipboard;

pub(crate) async fn start_network(state: &mut WorkspaceState) -> CoreResult<()> {
    let result = state.network_runtime().start().await.map_err(Into::into);
    finish_after_refresh(result, refresh_snapshot_from_runtime(state).await)
}

pub(crate) async fn stop_network(state: &mut WorkspaceState) -> CoreResult<()> {
    let result = state.network_runtime().shutdown().await.map_err(Into::into);
    finish_after_refresh(result, refresh_snapshot_from_runtime(state).await)
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
    finish_after_refresh(result, refresh_snapshot_from_runtime(state).await)
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
    finish_after_refresh(result, refresh_snapshot_from_runtime(state).await)
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
    finish_after_refresh(result, refresh_snapshot_from_runtime(state).await)
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
    if !snapshot_has_session(state.current_snapshot(), id) {
        return Err(CoreError::SessionNotFound {
            session_id: id.to_string(),
        });
    }

    let result = state
        .network_runtime()
        .disconnect_session(id)
        .await
        .map_err(Into::into);
    finish_after_refresh(result, refresh_snapshot_from_runtime(state).await)
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

pub(crate) async fn refresh_snapshot_from_runtime(state: &mut WorkspaceState) -> CoreResult<()> {
    let network_snapshot = state.network_runtime().snapshot().await?;
    state.refresh_snapshot(network_snapshot);
    Ok(())
}

fn finish_after_refresh<T>(result: CoreResult<T>, refresh: CoreResult<()>) -> CoreResult<T> {
    match result {
        Ok(value) => {
            refresh?;
            Ok(value)
        }
        Err(error) => Err(error),
    }
}

fn snapshot_has_session(snapshot: &crate::WorkspaceSnapshot, id: SessionId) -> bool {
    snapshot
        .network
        .sessions
        .iter()
        .any(|session| session.id == id)
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    use crate::{NetworkSnapshot, NetworkStatus, SessionId, SessionInfo};
    use nooboard_network::ConnectionMode;

    use super::snapshot_has_session;

    #[test]
    fn detects_session_presence_from_snapshot() {
        let session_id = SessionId::new();
        let snapshot = crate::WorkspaceSnapshot {
            revision: 0,
            identity: crate::WorkspaceIdentity {
                noob_id: crate::NoobId::new("local"),
                device_id: "device".to_string(),
            },
            local_connection: crate::LocalConnectionInfo::default(),
            clipboard: crate::ClipboardState::default(),
            settings: crate::WorkspaceSettings {
                connection: crate::ConnectionSettings {
                    device_id: "device".to_string(),
                    token: "token".to_string(),
                },
                network: crate::NetworkSettings {
                    listen_port: 1,
                    lan_enabled: false,
                },
                storage: crate::StorageSettings {
                    db_root: std::path::PathBuf::new(),
                    history_window_days: 1,
                    dedup_window_days: 1,
                    max_text_bytes: 1,
                    gc_batch_size: 1,
                },
                clipboard: crate::ClipboardSettings {
                    local_capture_enabled: false,
                },
                transfers: crate::TransferSettings {
                    download_dir: std::path::PathBuf::new(),
                },
            },
            network: NetworkSnapshot {
                status: NetworkStatus::Stopped,
                lan_enabled: false,
                lan_peers: Vec::new(),
                direct_seeds: Vec::new(),
                pending_direct_requests: Vec::new(),
                sessions: vec![SessionInfo {
                    id: session_id,
                    mode: ConnectionMode::Direct,
                    peer_noob_id: "peer".to_string(),
                    peer_device_id: "peer-device".to_string(),
                    remote_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 24000),
                    local_bind_addr: None,
                    outbound: true,
                    connected_at_ms: 0,
                }],
                transfers: Default::default(),
            },
        };

        assert!(snapshot_has_session(&snapshot, session_id));
        assert!(!snapshot_has_session(&snapshot, SessionId::new()));
    }
}
