use tokio::sync::mpsc;
use tokio::spawn;

use super::tasks::{classify_connection_error, now_millis};
use super::{ReadySession, RuntimeManager};
use crate::connection::policy::{DedupeDecision, dedupe_decision};
use crate::errors::{ConnectionError, NetworkResult};
use crate::session::actor::{SessionActorContext, SessionLifecycleEvent, run_session_actor};
use crate::{
    ConnectionFailure, ConnectionFailureKind, ConnectionMode, IncomingTransferDecision,
    NetworkEvent, SendFilesRequest, SendTextRequest, SessionId, SessionInfo, TransferTicket,
};

impl RuntimeManager {
    pub(crate) async fn list_sessions(&self) -> NetworkResult<Vec<SessionInfo>> {
        let state = self.inner.state.lock().await;
        Ok(state.list_sessions())
    }

    pub(crate) async fn disconnect_session(&self, id: SessionId) -> NetworkResult<()> {
        let mut state = self.inner.state.lock().await;
        state.disconnect_session(id)
    }

    pub(crate) async fn send_text(&self, request: SendTextRequest) -> NetworkResult<()> {
        let state = self.inner.state.lock().await;
        state.send_text(&request)
    }

    pub(crate) async fn send_files(
        &self,
        request: SendFilesRequest,
    ) -> NetworkResult<Vec<TransferTicket>> {
        let mut state = self.inner.state.lock().await;
        state.send_files(&request)
    }

    pub(crate) async fn decide_incoming_transfer(
        &self,
        decision: IncomingTransferDecision,
    ) -> NetworkResult<()> {
        let state = self.inner.state.lock().await;
        state.decide_incoming_transfer(decision).await
    }

    pub(crate) async fn cancel_transfer(&self, id: TransferTicket) -> NetworkResult<()> {
        let state = self.inner.state.lock().await;
        state.cancel_transfer(id).await
    }

    pub(super) async fn handle_session_lifecycle(&self, event: SessionLifecycleEvent) {
        match event {
            SessionLifecycleEvent::Network(NetworkEvent::IncomingTransferOffered { offer }) => {
                let mut state = self.inner.state.lock().await;
                state.apply_transfer_offer(offer.clone());
                drop(state);
                self.inner
                    .event_hub
                    .publish(NetworkEvent::IncomingTransferOffered { offer });
            }
            SessionLifecycleEvent::Network(NetworkEvent::TransferUpdated { transfer }) => {
                let mut state = self.inner.state.lock().await;
                state.apply_transfer_update(transfer.clone());
                drop(state);
                self.inner
                    .event_hub
                    .publish(NetworkEvent::TransferUpdated { transfer });
            }
            SessionLifecycleEvent::Network(NetworkEvent::TransferCompleted { transfer }) => {
                let mut state = self.inner.state.lock().await;
                state.apply_transfer_completed(transfer.clone());
                drop(state);
                self.inner
                    .event_hub
                    .publish(NetworkEvent::TransferCompleted { transfer });
            }
            SessionLifecycleEvent::Network(event) => {
                self.inner.event_hub.publish(event);
            }
            SessionLifecycleEvent::Failed { session_id, error } => {
                let removed = {
                    let mut state = self.inner.state.lock().await;
                    let info = state
                        .list_sessions()
                        .into_iter()
                        .find(|session| session.id == session_id);
                    let removed = state.remove_session_if_present(session_id);
                    (info, removed)
                };
                if removed.1 {
                    self.inner.event_hub.publish(NetworkEvent::SessionsChanged);
                    if removed
                        .0
                        .as_ref()
                        .is_some_and(|info| info.mode == ConnectionMode::Lan)
                    {
                        self.inner.event_hub.publish(NetworkEvent::LanPeersChanged);
                    }
                }
                if let Some(info) = removed.0 {
                    self.inner
                        .event_hub
                        .publish(NetworkEvent::ConnectionFailed(
                            connection_failure_from_session_error(info, error),
                        ));
                }
                self.schedule_lan_connects().await;
            }
            SessionLifecycleEvent::Disconnected { session_id } => {
                let removed_info = {
                    let mut state = self.inner.state.lock().await;
                    let info = state
                        .list_sessions()
                        .into_iter()
                        .find(|session| session.id == session_id);
                    let removed = state.remove_session_if_present(session_id);
                    removed.then_some(info).flatten()
                };
                if let Some(info) = removed_info {
                    self.inner.event_hub.publish(NetworkEvent::SessionsChanged);
                    if info.mode == ConnectionMode::Lan {
                        self.inner.event_hub.publish(NetworkEvent::LanPeersChanged);
                    }
                }
                self.schedule_lan_connects().await;
            }
        }
    }

    pub(super) async fn activate_session(&self, ready: ReadySession) {
        if ready.mode == ConnectionMode::Lan {
            match dedupe_decision(&self.inner.startup.identity.noob_id, &ready.peer_noob_id) {
                DedupeDecision::RejectConflict => {
                    self.inner
                        .event_hub
                        .publish(NetworkEvent::ConnectionFailed(ConnectionFailure {
                            kind: ConnectionFailureKind::AlreadyConnected,
                            mode: ready.mode,
                            peer_noob_id: Some(ready.peer_noob_id),
                            peer_device_id: Some(ready.peer_device_id),
                            remote_addr: Some(ready.remote_addr),
                            local_bind_addr: ready.local_bind_addr,
                            detail: "conflicting LAN noob_id".to_string(),
                        }));
                    return;
                }
                DedupeDecision::ConnectOut if !ready.outbound => return,
                DedupeDecision::WaitInbound if ready.outbound => return,
                _ => {}
            }
        }

        let duplicate_session = {
            let state = self.inner.state.lock().await;
            state.find_session_by_noob_id(&ready.peer_noob_id)
        };
        if duplicate_session.is_some() {
            self.inner
                .event_hub
                .publish(NetworkEvent::ConnectionFailed(ConnectionFailure {
                    kind: ConnectionFailureKind::AlreadyConnected,
                    mode: ready.mode,
                    peer_noob_id: Some(ready.peer_noob_id),
                    peer_device_id: Some(ready.peer_device_id),
                    remote_addr: Some(ready.remote_addr),
                    local_bind_addr: ready.local_bind_addr,
                    detail: "session already exists for peer".to_string(),
                }));
            return;
        }

        let session_id = SessionId::new();
        let info = SessionInfo {
            id: session_id,
            mode: ready.mode,
            peer_noob_id: ready.peer_noob_id.clone(),
            peer_device_id: ready.peer_device_id.clone(),
            remote_addr: ready.remote_addr,
            local_bind_addr: ready.local_bind_addr,
            outbound: ready.outbound,
            connected_at_ms: now_millis(),
        };
        let (command_tx, command_rx) = mpsc::channel(64);

        let running = match self.running_context().await {
            Ok(running) => running,
            Err(_) => return,
        };

        {
            let mut state = self.inner.state.lock().await;
            if let Some(seed_id) = ready.seed_id {
                state.record_direct_seed_success(seed_id, &ready.peer_device_id, ready.remote_addr);
            }
            if ready.mode == ConnectionMode::Lan {
                state.note_lan_connect_success(&ready.peer_noob_id);
            }
            state.insert_session(info, command_tx);
        }

        if ready.seed_id.is_some() {
            self.inner.event_hub.publish(NetworkEvent::DirectSeedsChanged);
        }
        if ready.mode == ConnectionMode::Lan {
            self.inner.event_hub.publish(NetworkEvent::LanPeersChanged);
        }
        self.inner.event_hub.publish(NetworkEvent::SessionsChanged);

        spawn(run_session_actor(SessionActorContext {
            session_id,
            peer_noob_id: ready.peer_noob_id,
            peer_device_id: ready.peer_device_id,
            config: self.inner.startup.runtime_config(),
            framed: ready.framed,
            command_rx,
            lifecycle_tx: running.lifecycle_tx,
            shutdown_rx: running.shutdown_tx.subscribe(),
        }));
    }
}

fn connection_failure_from_session_error(
    info: SessionInfo,
    error: ConnectionError,
) -> ConnectionFailure {
    ConnectionFailure {
        kind: classify_connection_error(&error),
        mode: info.mode,
        peer_noob_id: Some(info.peer_noob_id),
        peer_device_id: Some(info.peer_device_id),
        remote_addr: Some(info.remote_addr),
        local_bind_addr: info.local_bind_addr,
        detail: error.to_string(),
    }
}
