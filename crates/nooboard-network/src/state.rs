use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::NetworkConfig;
use crate::direct::approval_store::{ApprovalStore, PendingApproval};
use crate::direct::store::DirectSeedStore;
use crate::errors::{NetworkError, NetworkResult};
use crate::lan::peer_index::{LanPeerIndex, LanServiceRecord};
use crate::session::store::SessionStore;
use crate::transfer::store::TransferStore;
use crate::{
    ActiveTransferInfo, CompletedTransferInfo,
    DirectRequestId, DirectSeedId, DirectSeedInfo, IncomingTransferDecision,
    IncomingTransferDisposition, IncomingTransferOffer, NetworkEvent, NetworkSnapshot,
    NetworkStatus, PendingDirectRequest, SendFilesRequest, SendTextRequest, SessionId,
    SessionInfo, TransferTicket, UpsertDirectSeedInput,
};

pub(crate) struct RuntimeState {
    status: NetworkStatus,
    lan_enabled: bool,
    lan_peers: LanPeerIndex,
    direct_seeds: DirectSeedStore,
    pending_direct_requests: ApprovalStore,
    sessions: SessionStore,
    transfers: TransferStore,
}

pub(crate) enum DirectConnectPreparation {
    Start(DirectSeedInfo),
    AlreadyConnected(SessionId),
}

impl RuntimeState {
    pub(crate) fn new(config: &NetworkConfig) -> Self {
        Self {
            status: NetworkStatus::Stopped,
            lan_enabled: config.lan.enabled,
            lan_peers: LanPeerIndex::default(),
            direct_seeds: DirectSeedStore::from_configs(&config.direct.seeds),
            pending_direct_requests: ApprovalStore::default(),
            sessions: SessionStore::default(),
            transfers: TransferStore::default(),
        }
    }

    pub(crate) fn start(&mut self) -> Vec<NetworkEvent> {
        match self.status {
            NetworkStatus::Running | NetworkStatus::Starting => Vec::new(),
            NetworkStatus::Stopped | NetworkStatus::Error(_) => {
                self.status = NetworkStatus::Starting;
                vec![NetworkEvent::StatusChanged(NetworkStatus::Starting)]
            }
        }
    }

    pub(crate) fn mark_running(&mut self) -> Vec<NetworkEvent> {
        match self.status {
            NetworkStatus::Running => Vec::new(),
            _ => {
                self.status = NetworkStatus::Running;
                vec![NetworkEvent::StatusChanged(NetworkStatus::Running)]
            }
        }
    }

    pub(crate) fn set_error(&mut self, detail: String) -> Vec<NetworkEvent> {
        self.status = NetworkStatus::Error(detail.clone());
        vec![NetworkEvent::StatusChanged(NetworkStatus::Error(detail))]
    }

    pub(crate) fn shutdown(&mut self) -> Vec<NetworkEvent> {
        let already_stopped = matches!(self.status, NetworkStatus::Stopped);
        self.status = NetworkStatus::Stopped;
        self.lan_peers.clear();
        for approval in self.pending_direct_requests.clear() {
            approval.abort_timeout();
        }
        self.sessions = SessionStore::default();
        self.transfers.clear();

        if already_stopped {
            Vec::new()
        } else {
            vec![NetworkEvent::StatusChanged(NetworkStatus::Stopped)]
        }
    }

    pub(crate) fn snapshot(&self) -> NetworkSnapshot {
        NetworkSnapshot {
            status: self.status.clone(),
            lan_enabled: self.lan_enabled,
            lan_peers: self
                .lan_peers
                .snapshot(&self.sessions.connected_peer_noob_ids()),
            direct_seeds: self.direct_seeds.list(),
            pending_direct_requests: self.pending_direct_requests.list(),
            sessions: self.sessions.list_sorted(),
            transfers: self.transfers.snapshot(),
        }
    }

    pub(crate) fn set_lan_enabled(&mut self, enabled: bool) -> Vec<NetworkEvent> {
        self.lan_enabled = enabled;
        if enabled {
            return Vec::new();
        }

        self.lan_peers.clear();
        let removed = self.sessions.remove_lan_sessions();
        for session in &removed {
            self.transfers.remove_session(session.id);
        }

        let mut events = vec![NetworkEvent::LanPeersChanged];
        if !removed.is_empty() {
            events.push(NetworkEvent::SessionsChanged);
        }
        events
    }

    pub(crate) fn list_direct_seeds(&self) -> Vec<DirectSeedInfo> {
        self.direct_seeds.list()
    }

    pub(crate) fn upsert_direct_seed(
        &mut self,
        input: UpsertDirectSeedInput,
    ) -> NetworkResult<DirectSeedId> {
        self.direct_seeds.upsert(input)
    }

    pub(crate) fn remove_direct_seed(&mut self, id: DirectSeedId) -> NetworkResult<()> {
        if self.direct_seeds.remove(id) {
            Ok(())
        } else {
            Err(NetworkError::DirectSeedNotFound(id))
        }
    }

    pub(crate) fn search_direct_seeds(&self, query: &str) -> Vec<DirectSeedInfo> {
        self.direct_seeds.search(query)
    }

    pub(crate) fn prepare_direct_connect(
        &self,
        id: DirectSeedId,
    ) -> NetworkResult<DirectConnectPreparation> {
        if !matches!(self.status, NetworkStatus::Running) {
            return Err(NetworkError::NotRunning);
        }

        let seed = self
            .direct_seeds
            .get_cloned(id)
            .ok_or(NetworkError::DirectSeedNotFound(id))?;
        if !seed.enabled {
            return Err(NetworkError::Internal(format!(
                "direct seed is disabled: {id}"
            )));
        }

        if let Some(remote_addr) = self.direct_seeds.resolve_ipv4_addr(id)
            && let Some(existing) = self.sessions.find_direct_by_remote_addr(remote_addr)
        {
            return Ok(DirectConnectPreparation::AlreadyConnected(existing));
        }

        Ok(DirectConnectPreparation::Start(seed))
    }

    pub(crate) fn list_pending_direct_requests(&self) -> Vec<PendingDirectRequest> {
        self.pending_direct_requests.list()
    }

    pub(crate) fn take_pending_direct_request(
        &mut self,
        id: DirectRequestId,
    ) -> NetworkResult<PendingApproval> {
        self.pending_direct_requests
            .remove(id)
            .ok_or(NetworkError::DirectRequestNotFound(id))
    }

    pub(crate) fn list_sessions(&self) -> Vec<SessionInfo> {
        self.sessions.list_sorted()
    }

    pub(crate) fn disconnect_session(&mut self, id: SessionId) -> NetworkResult<()> {
        self.sessions.disconnect(id)
    }

    pub(crate) fn send_text(&self, request: &SendTextRequest) -> NetworkResult<()> {
        if !matches!(self.status, NetworkStatus::Running) {
            return Err(NetworkError::NotRunning);
        }
        self.sessions.send_text(request)
    }

    pub(crate) fn send_files(
        &mut self,
        request: &SendFilesRequest,
    ) -> NetworkResult<Vec<TransferTicket>> {
        if !matches!(self.status, NetworkStatus::Running) {
            return Err(NetworkError::NotRunning);
        }
        self.sessions.send_files(request)
    }

    pub(crate) fn validate_incoming_transfer_decision(
        &self,
        decision: &IncomingTransferDecision,
    ) -> NetworkResult<()> {
        if !matches!(self.status, NetworkStatus::Running) {
            return Err(NetworkError::NotRunning);
        }
        if self.transfers.has_pending_offer(decision.ticket) {
            Ok(())
        } else {
            Err(NetworkError::Internal(format!(
                "incoming transfer offer not found: {}:{}",
                decision.ticket.session_id, decision.ticket.raw_id
            )))
        }
    }

    pub(crate) async fn decide_incoming_transfer(
        &self,
        decision: IncomingTransferDecision,
    ) -> NetworkResult<()> {
        self.validate_incoming_transfer_decision(&decision)?;
        self.sessions.decide_incoming_transfer(
            decision.ticket,
            matches!(decision.decision, IncomingTransferDisposition::Accept),
            match decision.decision {
                IncomingTransferDisposition::Accept => None,
                IncomingTransferDisposition::Reject => Some("rejected by local peer".to_string()),
            },
        )
    }

    pub(crate) async fn cancel_transfer(&self, id: TransferTicket) -> NetworkResult<()> {
        if !matches!(self.status, NetworkStatus::Running) {
            return Err(NetworkError::NotRunning);
        }
        if self.transfers.has_active_or_pending(id) {
            self.sessions.cancel_transfer(id).await
        } else {
            Err(NetworkError::Internal(format!(
                "transfer not found: {}:{}",
                id.session_id, id.raw_id
            )))
        }
    }

    pub(crate) fn add_pending_direct_request(&mut self, approval: PendingApproval) {
        self.pending_direct_requests.insert(approval);
    }

    pub(crate) fn has_pending_direct_request_for_peer(&self, peer_noob_id: &str) -> bool {
        self.pending_direct_requests.contains_peer_noob_id(peer_noob_id)
    }

    pub(crate) fn record_direct_seed_success(
        &mut self,
        id: DirectSeedId,
        device_id: &str,
        remote_addr: std::net::SocketAddr,
    ) {
        self.direct_seeds.record_success(id, device_id, remote_addr);
    }

    pub(crate) fn insert_session(
        &mut self,
        info: SessionInfo,
        command_tx: tokio::sync::mpsc::Sender<crate::session::actor::SessionCommand>,
    ) {
        self.sessions.insert(info, command_tx);
    }

    pub(crate) fn remove_session_if_present(&mut self, id: SessionId) -> bool {
        let removed = self.sessions.remove(id).is_some();
        if removed {
            self.transfers.remove_session(id);
        }
        removed
    }

    pub(crate) fn shutdown_all_sessions(&self) {
        self.sessions.shutdown_all();
    }

    pub(crate) fn find_session_by_noob_id(&self, peer_noob_id: &str) -> Option<SessionId> {
        self.sessions.find_by_peer_noob_id(peer_noob_id)
    }

    pub(crate) fn find_direct_session_by_addr(
        &self,
        remote_addr: std::net::SocketAddr,
    ) -> Option<SessionId> {
        self.sessions.find_direct_by_remote_addr(remote_addr)
    }

    pub(crate) fn apply_transfer_offer(&mut self, offer: IncomingTransferOffer) {
        self.transfers.apply_offer(offer);
    }

    pub(crate) fn apply_transfer_update(&mut self, transfer: ActiveTransferInfo) {
        self.transfers.apply_update(transfer);
    }

    pub(crate) fn apply_transfer_completed(&mut self, transfer: CompletedTransferInfo) {
        self.transfers.apply_completed(transfer);
    }

    pub(crate) fn apply_lan_service_resolved(&mut self, record: LanServiceRecord) {
        self.lan_peers.apply_resolved(record);
    }

    pub(crate) fn apply_lan_service_removed(&mut self, fullname: &str) -> bool {
        self.lan_peers.apply_removed(fullname)
    }

    pub(crate) fn next_lan_candidate(&self, local_noob_id: &str, now_ms: u64) -> Option<crate::lan::peer_index::LanConnectCandidate> {
        let connected = self.sessions.connected_peer_noob_ids();
        self.lan_peers.next_candidate(local_noob_id, &connected, now_ms)
    }

    pub(crate) fn mark_lan_connecting(&mut self, peer_noob_id: &str) {
        self.lan_peers.mark_connecting(peer_noob_id);
    }

    pub(crate) fn note_lan_connect_success(&mut self, peer_noob_id: &str) {
        self.lan_peers.note_connect_success(peer_noob_id);
    }

    pub(crate) fn note_lan_connect_finished(&mut self, peer_noob_id: &str) {
        self.lan_peers.note_connect_finished(peer_noob_id);
    }

    pub(crate) fn note_lan_connect_failure(&mut self, peer_noob_id: &str, now_ms: u64) {
        self.lan_peers.note_connect_failure(peer_noob_id, now_ms);
    }
}

#[allow(dead_code)]
fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use tokio::sync::mpsc;

    use super::*;
    use crate::config::{
        DirectConfig, LanConfig, LocalIdentityConfig, NetworkAuthConfig, NetworkTransferConfig,
        NetworkTransportConfig,
    };
    use crate::ConnectionMode;

    fn test_config() -> NetworkConfig {
        NetworkConfig {
            identity: LocalIdentityConfig {
                noob_id: "node-a".to_string(),
                device_id: "device-a".to_string(),
            },
            listen_port: 17890,
            auth: NetworkAuthConfig {
                token: "token".to_string(),
            },
            lan: LanConfig { enabled: true },
            direct: DirectConfig {
                approval_timeout_ms: 30_000,
                seeds: vec![],
            },
            transport: NetworkTransportConfig {
                connect_timeout_ms: 5_000,
                handshake_timeout_ms: 5_000,
                ping_interval_ms: 5_000,
                pong_timeout_ms: 15_000,
                max_packet_size: 8 * 1024 * 1024,
            },
            transfer: NetworkTransferConfig {
                download_dir: PathBuf::from("/tmp"),
                max_file_size: 1024,
                chunk_size: 512,
                active_downloads: 1,
                decision_timeout_ms: 30_000,
                idle_timeout_ms: 15_000,
            },
        }
    }

    #[test]
    fn snapshot_sorts_sessions_and_respects_lan_disable() {
        let mut state = RuntimeState::new(&test_config());
        state.status = NetworkStatus::Running;
        let (tx1, _) = mpsc::channel(1);
        let (tx2, _) = mpsc::channel(1);
        state.sessions.insert(SessionInfo {
            id: SessionId::new(),
            mode: ConnectionMode::Direct,
            peer_noob_id: "b".to_string(),
            peer_device_id: "device-b".to_string(),
            remote_addr: "127.0.0.1:1001".parse().expect("addr"),
            local_bind_addr: None,
            outbound: true,
            connected_at_ms: 2,
        }, tx1);
        state.sessions.insert(SessionInfo {
            id: SessionId::new(),
            mode: ConnectionMode::Lan,
            peer_noob_id: "a".to_string(),
            peer_device_id: "device-a".to_string(),
            remote_addr: "127.0.0.1:1000".parse().expect("addr"),
            local_bind_addr: None,
            outbound: false,
            connected_at_ms: 1,
        }, tx2);

        let before = state.snapshot();
        assert_eq!(before.sessions.len(), 2);
        assert_eq!(before.sessions[0].connected_at_ms, 1);

        state.set_lan_enabled(false);
        let after = state.snapshot();
        assert!(!after.lan_enabled);
        assert_eq!(after.sessions.len(), 1);
        assert_eq!(after.sessions[0].mode, ConnectionMode::Direct);
    }

}
