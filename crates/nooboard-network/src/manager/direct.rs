use std::net::{SocketAddr, SocketAddrV4};
use std::time::Duration;

use tokio::spawn;

use super::tasks::{classify_transport_error, connect_outbound, now_millis};
use super::{InternalEvent, ReadySession, RunningContext, RuntimeManager};
use crate::direct::approval_store::PendingApproval;
use crate::direct::resolver::{ResolveError, resolve_seed_ipv4_addrs};
use crate::protocol::{ConnectionIntent, ControlPacket, DirectRequestStatus, Packet};
use crate::state::DirectConnectPreparation;
use crate::transport::{recv_packet, send_packet};
use crate::{
    ConnectDirectOutcome, ConnectionFailure, ConnectionFailureKind, ConnectionMode,
    DirectRequestId, DirectSeedId, DirectSeedInfo, NetworkEvent, NetworkResult,
    PendingDirectRequest, UpsertDirectSeedInput,
};
use crate::errors::TransportError;

impl RuntimeManager {
    pub(crate) async fn list_direct_seeds(&self) -> NetworkResult<Vec<crate::DirectSeedInfo>> {
        let state = self.inner.state.lock().await;
        Ok(state.list_direct_seeds())
    }

    pub(crate) async fn upsert_direct_seed(
        &self,
        input: UpsertDirectSeedInput,
    ) -> NetworkResult<DirectSeedId> {
        let id = {
            let mut state = self.inner.state.lock().await;
            state.upsert_direct_seed(input)?
        };
        self.inner.event_hub.publish(NetworkEvent::DirectSeedsChanged);
        Ok(id)
    }

    pub(crate) async fn remove_direct_seed(&self, id: DirectSeedId) -> NetworkResult<()> {
        {
            let mut state = self.inner.state.lock().await;
            state.remove_direct_seed(id)?;
        }
        self.inner.event_hub.publish(NetworkEvent::DirectSeedsChanged);
        Ok(())
    }

    pub(crate) async fn search_direct_seeds(
        &self,
        query: &str,
    ) -> NetworkResult<Vec<crate::DirectSeedInfo>> {
        let state = self.inner.state.lock().await;
        Ok(state.search_direct_seeds(query))
    }

    pub(crate) async fn connect_direct_seed(
        &self,
        id: DirectSeedId,
    ) -> NetworkResult<ConnectDirectOutcome> {
        let seed = {
            let state = self.inner.state.lock().await;
            match state.prepare_direct_connect(id)? {
                DirectConnectPreparation::AlreadyConnected(session_id) => {
                    return Ok(ConnectDirectOutcome::AlreadyConnected(session_id));
                }
                DirectConnectPreparation::Start(seed) => seed,
            }
        };

        let context = self.running_context().await?;
        let addrs = match resolve_seed_ipv4_addrs(&seed).await {
            Ok(addrs) => addrs,
            Err(error) => {
                self.inner.event_hub.publish(connection_failure_from_resolve(
                    ConnectionMode::Direct,
                    &seed,
                    error,
                ));
                return Ok(ConnectDirectOutcome::Started);
            }
        };

        {
            let state = self.inner.state.lock().await;
            if let Some(existing) = addrs
                .iter()
                .find_map(|addr| state.find_direct_session_by_addr(*addr))
            {
                return Ok(ConnectDirectOutcome::AlreadyConnected(existing));
            }
        }

        let manager = self.clone();
        let primary_addr = addrs[0];
        let task = spawn(async move {
            run_direct_connect_task(manager, context, id, seed, addrs).await;
        });

        {
            let mut driver = self.inner.driver.lock().await;
            match driver
                .coordinator
                .begin(ConnectionMode::Direct, primary_addr, task)
            {
                crate::connection::coordinator::BeginAttempt::Granted => {}
                crate::connection::coordinator::BeginAttempt::Skipped => {
                    return Ok(ConnectDirectOutcome::Started);
                }
            }
        }

        Ok(ConnectDirectOutcome::Started)
    }

    pub(crate) async fn list_pending_direct_requests(
        &self,
    ) -> NetworkResult<Vec<PendingDirectRequest>> {
        let state = self.inner.state.lock().await;
        Ok(state.list_pending_direct_requests())
    }

    pub(crate) async fn approve_direct_request(&self, id: DirectRequestId) -> NetworkResult<()> {
        let pending = {
            let mut state = self.inner.state.lock().await;
            state.take_pending_direct_request(id)?
        };
        pending.abort_timeout();

        self.inner
            .event_hub
            .publish(NetworkEvent::PendingDirectRequestsChanged);

        let mut framed = pending.framed;
        let _ = send_direct_status(
            &mut framed,
            pending.info.id,
            DirectRequestStatus::Approved,
            None,
            None,
        )
        .await;

        self.activate_session(ReadySession {
            mode: ConnectionMode::Direct,
            seed_id: None,
            remote_addr: pending.info.remote_addr,
            local_bind_addr: pending.local_bind_addr,
            peer_noob_id: pending.info.peer_noob_id,
            peer_device_id: pending.info.peer_device_id,
            outbound: false,
            framed,
        })
        .await;
        Ok(())
    }

    pub(crate) async fn reject_direct_request(&self, id: DirectRequestId) -> NetworkResult<()> {
        let pending = {
            let mut state = self.inner.state.lock().await;
            state.take_pending_direct_request(id)?
        };
        pending.abort_timeout();

        self.inner
            .event_hub
            .publish(NetworkEvent::PendingDirectRequestsChanged);

        let mut framed = pending.framed;
        let _ = send_direct_status(
            &mut framed,
            pending.info.id,
            DirectRequestStatus::Rejected,
            None,
            Some("rejected by remote peer".to_string()),
        )
        .await;
        Ok(())
    }

    pub(super) async fn handle_inbound_direct(&self, pending: super::InboundDirectPending) {
        let duplicate = {
            let state = self.inner.state.lock().await;
            state.find_session_by_noob_id(&pending.peer_noob_id).is_some()
                || state.has_pending_direct_request_for_peer(&pending.peer_noob_id)
        };
        if duplicate {
            let mut framed = pending.framed;
            let _ = send_direct_status(
                &mut framed,
                DirectRequestId::new(),
                DirectRequestStatus::AlreadyConnected,
                None,
                Some("already connected".to_string()),
            )
            .await;
            return;
        }

        let request_id = DirectRequestId::new();
        let expires_at_ms = now_millis() + self.inner.startup.approval_timeout_ms;
        let mut framed = pending.framed;
        if send_direct_status(
            &mut framed,
            request_id,
            DirectRequestStatus::PendingApproval,
            Some(expires_at_ms),
            None,
        )
        .await
        .is_err()
        {
            return;
        }

        let internal_tx = match self.running_context().await {
            Ok(running) => running.internal_tx,
            Err(_) => return,
        };
        let timeout_task = spawn(async move {
            tokio::time::sleep(Duration::from_millis(
                expires_at_ms.saturating_sub(now_millis()),
            ))
            .await;
            let _ = internal_tx
                .send(InternalEvent::ApprovalExpired(request_id))
                .await;
        });

        {
            let mut state = self.inner.state.lock().await;
            state.add_pending_direct_request(PendingApproval {
                info: PendingDirectRequest {
                    id: request_id,
                    remote_addr: pending.remote_addr,
                    peer_noob_id: pending.peer_noob_id,
                    peer_device_id: pending.peer_device_id,
                    expires_at_ms,
                },
                local_bind_addr: pending.local_bind_addr,
                framed,
                timeout_task,
            });
        }
        self.inner
            .event_hub
            .publish(NetworkEvent::PendingDirectRequestsChanged);
    }

    pub(super) async fn expire_direct_request(&self, id: DirectRequestId) {
        let pending = {
            let mut state = self.inner.state.lock().await;
            state.take_pending_direct_request(id).ok()
        };
        let Some(pending) = pending else {
            return;
        };
        let mut framed = pending.framed;
        let _ = send_direct_status(
            &mut framed,
            pending.info.id,
            DirectRequestStatus::Expired,
            None,
            Some("direct approval expired".to_string()),
        )
        .await;
        self.inner
            .event_hub
            .publish(NetworkEvent::PendingDirectRequestsChanged);
    }
}

pub(super) async fn run_direct_connect_task(
    manager: RuntimeManager,
    context: RunningContext,
    seed_id: DirectSeedId,
    seed: crate::DirectSeedInfo,
    addrs: Vec<SocketAddr>,
) {
    let mut first_addr = None;
    for addr in addrs {
        if first_addr.is_none() {
            first_addr = Some(addr);
        }

        match connect_outbound(
            manager.inner.startup.runtime_config(),
            &context.tls,
            ConnectionIntent::DirectConnect,
            addr,
        )
        .await
        {
            Ok((peer, local_bind_addr, mut framed)) => {
                let outcome = wait_for_direct_approval(&mut framed).await;
                match outcome {
                    Ok(DirectRequestStatus::Approved) => {
                        let _ = context
                            .internal_tx
                            .send(InternalEvent::SessionReady(ReadySession {
                                mode: ConnectionMode::Direct,
                                seed_id: Some(seed_id),
                                remote_addr: addr,
                                local_bind_addr: Some(local_bind_addr),
                                peer_noob_id: peer.noob_id,
                                peer_device_id: peer.device_id,
                                outbound: true,
                                framed,
                            }))
                            .await;
                        manager
                            .finish_outbound_attempt(
                                ConnectionMode::Direct,
                                first_addr.unwrap_or(addr),
                            )
                            .await;
                        return;
                    }
                    Ok(status) => {
                        let _ = context
                            .internal_tx
                            .send(InternalEvent::ConnectFailed(ConnectionFailure {
                                kind: match status {
                                    DirectRequestStatus::Rejected => {
                                        ConnectionFailureKind::DirectRejected
                                    }
                                    DirectRequestStatus::Expired => {
                                        ConnectionFailureKind::DirectExpired
                                    }
                                    DirectRequestStatus::AlreadyConnected => {
                                        ConnectionFailureKind::AlreadyConnected
                                    }
                                    DirectRequestStatus::PendingApproval
                                    | DirectRequestStatus::Approved => {
                                        ConnectionFailureKind::Internal
                                    }
                                },
                                mode: ConnectionMode::Direct,
                                peer_noob_id: Some(peer.noob_id),
                                peer_device_id: Some(peer.device_id),
                                remote_addr: Some(addr),
                                local_bind_addr: Some(local_bind_addr),
                                detail: format!("direct connect status: {status:?}"),
                            }))
                            .await;
                        manager
                            .finish_outbound_attempt(
                                ConnectionMode::Direct,
                                first_addr.unwrap_or(addr),
                            )
                            .await;
                        return;
                    }
                    Err(failure) => {
                        let _ = context
                            .internal_tx
                            .send(InternalEvent::ConnectFailed(failure))
                            .await;
                    }
                }
            }
            Err(failure) => {
                let _ = context
                    .internal_tx
                    .send(InternalEvent::ConnectFailed(failure))
                    .await;
            }
        }
    }
    manager
        .finish_outbound_attempt(
            ConnectionMode::Direct,
            first_addr.unwrap_or_else(|| {
                SocketAddr::V4(SocketAddrV4::new(std::net::Ipv4Addr::LOCALHOST, seed.port))
            }),
        )
        .await;
}

async fn wait_for_direct_approval(
    framed: &mut crate::transport::NetworkFramed,
) -> Result<DirectRequestStatus, ConnectionFailure> {
    loop {
        let Some(packet) = recv_packet(framed).await.map_err(|error| ConnectionFailure {
            kind: classify_transport_error(&error),
            mode: ConnectionMode::Direct,
            peer_noob_id: None,
            peer_device_id: None,
            remote_addr: None,
            local_bind_addr: None,
            detail: error.to_string(),
        })? else {
            return Err(ConnectionFailure {
                kind: ConnectionFailureKind::Io,
                mode: ConnectionMode::Direct,
                peer_noob_id: None,
                peer_device_id: None,
                remote_addr: None,
                local_bind_addr: None,
                detail: "connection closed while waiting direct approval".to_string(),
            });
        };

        match packet {
            Packet::Control(ControlPacket::DirectRequestStatus { status, .. }) => match status {
                DirectRequestStatus::PendingApproval => continue,
                other => return Ok(other),
            },
            Packet::Ping { timestamp } => {
                send_packet(framed, &Packet::Pong { timestamp })
                    .await
                    .map_err(|error| ConnectionFailure {
                        kind: classify_transport_error(&error),
                        mode: ConnectionMode::Direct,
                        peer_noob_id: None,
                        peer_device_id: None,
                        remote_addr: None,
                        local_bind_addr: None,
                        detail: error.to_string(),
                    })?;
            }
            Packet::Pong { .. } => {}
            Packet::Control(_) => {}
            Packet::Handshake(_) | Packet::Data(_) => {
                return Err(ConnectionFailure {
                    kind: ConnectionFailureKind::Internal,
                    mode: ConnectionMode::Direct,
                    peer_noob_id: None,
                    peer_device_id: None,
                    remote_addr: None,
                    local_bind_addr: None,
                    detail: "unexpected packet while waiting direct approval".to_string(),
                });
            }
        }
    }
}

async fn send_direct_status(
    framed: &mut crate::transport::NetworkFramed,
    request_id: DirectRequestId,
    status: DirectRequestStatus,
    expires_at_ms: Option<u64>,
    reason: Option<String>,
) -> Result<(), TransportError> {
    send_packet(
        framed,
        &Packet::Control(ControlPacket::DirectRequestStatus {
            request_id,
            status,
            expires_at_ms,
            reason,
        }),
    )
    .await
}

fn connection_failure_from_resolve(
    mode: ConnectionMode,
    seed: &DirectSeedInfo,
    error: ResolveError,
) -> NetworkEvent {
    NetworkEvent::ConnectionFailed(ConnectionFailure {
        kind: ConnectionFailureKind::ResolveFailed,
        mode,
        peer_noob_id: None,
        peer_device_id: seed.learned_device_id.clone(),
        remote_addr: seed.last_connected_addr,
        local_bind_addr: None,
        detail: error.to_string(),
    })
}
