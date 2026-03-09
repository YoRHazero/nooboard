use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;

use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot};
use tokio_rustls::TlsStream;
use tokio_util::codec::{Framed, LengthDelimitedCodec};

use crate::discovery::DiscoveredPeer;
use crate::error::{ConnectionError, SyncError};
use crate::session::actor::SessionCommand;

use super::candidates::{CandidateRegistry, ConnectTarget};
use super::policy::DedupeDecision;
use super::types::{
    CancelTransferRequest, ConnectedPeerInfo, FileDecisionInput, PeerConnectionState,
    ScheduledTransfer, SendFileRequest, SendTextRequest,
};

#[derive(Debug)]
pub(super) struct PeerHandle {
    pub(super) command_tx: mpsc::Sender<SessionCommand>,
    pub(super) addr: SocketAddr,
    pub(super) outbound: bool,
    pub(super) device_id: String,
    pub(super) session_id: u64,
    pub(super) connected_at_ms: u64,
}

#[derive(Debug)]
pub(super) enum EngineControl {
    Connected {
        peer_noob_id: String,
        peer_device_id: String,
        addr: SocketAddr,
        outbound: bool,
        framed: Framed<TlsStream<TcpStream>, LengthDelimitedCodec>,
    },
    ConnectFailed {
        addr: SocketAddr,
        error: SyncError,
    },
    ConnectAttemptFinished {
        addr: SocketAddr,
    },
    PeerFailed {
        peer_noob_id: String,
        session_id: u64,
        error: ConnectionError,
    },
    PeerDisconnected {
        peer_noob_id: String,
        session_id: u64,
    },
    DiscoveredPeer(DiscoveredPeer),
}

#[derive(Debug, Default)]
pub(super) struct PeerRegistry {
    peers: HashMap<String, PeerHandle>,
    connecting_addrs: HashSet<SocketAddr>,
    candidates: CandidateRegistry,
    next_transfer_id_by_peer: HashMap<String, u32>,
}

impl PeerRegistry {
    pub(super) fn new() -> Self {
        Self::default()
    }

    pub(super) fn clear_connecting(&mut self, addr: &SocketAddr) {
        self.connecting_addrs.remove(addr);
    }

    pub(super) fn mark_connecting(&mut self, addr: SocketAddr) {
        self.connecting_addrs.insert(addr);
    }

    pub(super) fn connect_targets(&self, manual_peers: &[SocketAddr]) -> Vec<ConnectTarget> {
        self.candidates.connect_targets(manual_peers)
    }

    pub(super) fn should_skip_target(&self, addr: &SocketAddr) -> bool {
        self.connecting_addrs.contains(addr) || self.peers.values().any(|peer| peer.addr == *addr)
    }

    pub(super) fn send_text(&self, request: SendTextRequest) {
        let SendTextRequest {
            event_id,
            content,
            targets,
        } = request;
        let session_request = SendTextRequest {
            event_id,
            content,
            targets: None,
        };

        match targets {
            None => {
                for (peer_noob_id, peer) in &self.peers {
                    let _ = try_send_session_command(
                        &peer.command_tx,
                        SessionCommand::SendText(session_request.clone()),
                        peer_noob_id,
                        "send text",
                    );
                }
            }
            Some(targets) => {
                let mut deduped = HashSet::new();
                for target in targets {
                    if !deduped.insert(target.clone()) {
                        continue;
                    }
                    if let Some(peer) = self.peers.get(&target) {
                        let _ = try_send_session_command(
                            &peer.command_tx,
                            SessionCommand::SendText(session_request.clone()),
                            &target,
                            "send text",
                        );
                    }
                }
            }
        }
    }

    pub(super) fn send_file(
        &mut self,
        request: SendFileRequest,
    ) -> Result<Vec<ScheduledTransfer>, ConnectionError> {
        let SendFileRequest { path, targets } = request;
        let target_ids = self.resolve_targets(targets)?;
        let mut reservations = Vec::with_capacity(target_ids.len());

        for target in &target_ids {
            let peer = self
                .peers
                .get(target)
                .ok_or_else(|| ConnectionError::State(format!("peer {target} is not connected")))?;
            let command_tx = peer.command_tx.clone();
            let permit = command_tx
                .try_reserve_owned()
                .map_err(|error| match error {
                    mpsc::error::TrySendError::Full(_) => ConnectionError::State(format!(
                        "peer {target} session queue is full while send file"
                    )),
                    mpsc::error::TrySendError::Closed(_) => ConnectionError::State(format!(
                        "peer {target} session queue is closed while send file"
                    )),
                })?;
            let transfer_id = *self
                .next_transfer_id_by_peer
                .entry(target.clone())
                .or_insert(1);
            reservations.push((target.clone(), transfer_id, permit));
        }

        let mut scheduled = Vec::with_capacity(reservations.len());
        for (target, transfer_id, permit) in reservations {
            permit.send(SessionCommand::SendFile {
                transfer_id,
                path: path.clone(),
            });
            let entry = self
                .next_transfer_id_by_peer
                .entry(target.clone())
                .or_insert(1);
            *entry = transfer_id.wrapping_add(1);
            scheduled.push(ScheduledTransfer {
                peer_noob_id: target,
                transfer_id,
            });
        }

        Ok(scheduled)
    }

    pub(super) fn forward_file_decision(
        &self,
        decision: FileDecisionInput,
    ) -> Result<(), ConnectionError> {
        if let Some(peer) = self.peers.get(&decision.peer_noob_id) {
            try_send_session_command(
                &peer.command_tx,
                SessionCommand::FileDecision {
                    transfer_id: decision.transfer_id,
                    accept: decision.accept,
                    reason: decision.reason,
                },
                &decision.peer_noob_id,
                "forward file decision",
            )?;
            Ok(())
        } else {
            Err(ConnectionError::State(format!(
                "peer {} is not connected",
                decision.peer_noob_id
            )))
        }
    }

    pub(super) fn disconnect_peer(&mut self, peer_noob_id: &str) -> Option<SocketAddr> {
        let peer = self.peers.remove(peer_noob_id)?;
        let addr = peer.addr;
        let _ = try_send_session_command(
            &peer.command_tx,
            SessionCommand::Shutdown,
            peer_noob_id,
            "disconnect peer",
        );
        Some(addr)
    }

    pub(super) fn shutdown_all(&self) {
        for (peer_noob_id, peer) in &self.peers {
            let _ = try_send_session_command(
                &peer.command_tx,
                SessionCommand::Shutdown,
                peer_noob_id,
                "shutdown engine",
            );
        }
    }

    pub(super) fn remove_peer_if_session(&mut self, peer_noob_id: &str, session_id: u64) -> bool {
        let should_remove = self
            .peers
            .get(peer_noob_id)
            .map(|peer| peer.session_id == session_id)
            .unwrap_or(false);
        if should_remove {
            self.peers.remove(peer_noob_id);
            true
        } else {
            false
        }
    }

    pub(super) fn insert_peer(&mut self, peer_noob_id: String, handle: PeerHandle) {
        self.next_transfer_id_by_peer
            .entry(peer_noob_id.clone())
            .or_insert(1);
        self.peers.insert(peer_noob_id, handle);
    }

    pub(super) async fn cancel_transfer(
        &self,
        request: CancelTransferRequest,
    ) -> Result<(), ConnectionError> {
        let peer = self.peers.get(&request.peer_noob_id).ok_or_else(|| {
            ConnectionError::State(format!("peer {} is not connected", request.peer_noob_id))
        })?;
        let (reply_tx, reply_rx) = oneshot::channel();
        try_send_session_command(
            &peer.command_tx,
            SessionCommand::CancelTransfer {
                transfer_id: request.transfer_id,
                reply: reply_tx,
            },
            &request.peer_noob_id,
            "cancel transfer",
        )?;
        reply_rx.await.map_err(|_| {
            ConnectionError::State(format!(
                "peer {} session dropped cancel transfer reply",
                request.peer_noob_id
            ))
        })?
    }

    pub(super) fn peer_outbound(&self, peer_noob_id: &str) -> Option<bool> {
        self.peers.get(peer_noob_id).map(|peer| peer.outbound)
    }

    pub(super) fn peer_command_tx(
        &self,
        peer_noob_id: &str,
    ) -> Option<mpsc::Sender<SessionCommand>> {
        self.peers
            .get(peer_noob_id)
            .map(|peer| peer.command_tx.clone())
    }

    pub(super) fn peer_matches_session(&self, peer_noob_id: &str, session_id: u64) -> bool {
        self.peers
            .get(peer_noob_id)
            .map(|peer| peer.session_id == session_id)
            .unwrap_or(false)
    }

    pub(super) fn snapshot(&self) -> Vec<ConnectedPeerInfo> {
        let mut peers: Vec<ConnectedPeerInfo> = self
            .peers
            .iter()
            .map(|(peer_noob_id, handle)| ConnectedPeerInfo {
                peer_noob_id: peer_noob_id.clone(),
                peer_device_id: handle.device_id.clone(),
                addr: handle.addr,
                outbound: handle.outbound,
                connected_at_ms: handle.connected_at_ms,
                state: PeerConnectionState::Connected,
            })
            .collect();
        peers.sort_unstable_by(|left, right| left.peer_noob_id.cmp(&right.peer_noob_id));
        peers
    }

    pub(super) fn clear_peers(&mut self) {
        self.peers.clear();
    }

    pub(super) fn apply_discovered_peer(
        &mut self,
        local_noob_id: &str,
        peer: &DiscoveredPeer,
    ) -> DedupeDecision {
        self.candidates.apply_discovered_peer(local_noob_id, peer)
    }

    fn resolve_targets(
        &self,
        targets: Option<Vec<String>>,
    ) -> Result<Vec<String>, ConnectionError> {
        match targets {
            None => {
                if self.peers.is_empty() {
                    return Err(ConnectionError::State(
                        "no connected peers available for file transfer".to_string(),
                    ));
                }
                Ok(self.peers.keys().cloned().collect())
            }
            Some(targets) => {
                let mut deduped = HashSet::new();
                let mut resolved = Vec::new();
                for target in targets {
                    if !deduped.insert(target.clone()) {
                        continue;
                    }
                    if !self.peers.contains_key(&target) {
                        return Err(ConnectionError::State(format!(
                            "peer {target} is not connected"
                        )));
                    }
                    resolved.push(target);
                }
                if resolved.is_empty() {
                    return Err(ConnectionError::State(
                        "no connected peers available for file transfer".to_string(),
                    ));
                }
                Ok(resolved)
            }
        }
    }
}

fn try_send_session_command(
    command_tx: &mpsc::Sender<SessionCommand>,
    command: SessionCommand,
    peer_noob_id: &str,
    op: &'static str,
) -> Result<(), ConnectionError> {
    command_tx.try_send(command).map_err(|error| match error {
        mpsc::error::TrySendError::Full(_) => ConnectionError::State(format!(
            "peer {peer_noob_id} session queue is full while {op}"
        )),
        mpsc::error::TrySendError::Closed(_) => ConnectionError::State(format!(
            "peer {peer_noob_id} session queue is closed while {op}"
        )),
    })
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use tokio::time::timeout;

    fn insert_peer_for_test(
        registry: &mut PeerRegistry,
        peer_noob_id: &str,
    ) -> mpsc::Receiver<SessionCommand> {
        let (command_tx, command_rx) = mpsc::channel(4);
        registry.insert_peer(
            peer_noob_id.to_string(),
            PeerHandle {
                command_tx,
                addr: "127.0.0.1:10001"
                    .parse()
                    .expect("test addr should be valid"),
                outbound: true,
                device_id: peer_noob_id.to_string(),
                session_id: 1,
                connected_at_ms: 1,
            },
        );
        command_rx
    }

    #[tokio::test]
    async fn forward_file_decision_returns_error_when_peer_missing() {
        let registry = PeerRegistry::new();
        let result = registry.forward_file_decision(FileDecisionInput {
            peer_noob_id: "missing-peer".to_string(),
            transfer_id: 1,
            accept: true,
            reason: None,
        });

        assert!(matches!(result, Err(ConnectionError::State(_))));
    }

    #[tokio::test]
    async fn forward_file_decision_returns_error_when_session_channel_closed() {
        let mut registry = PeerRegistry::new();
        let (command_tx, command_rx) = mpsc::channel(1);
        drop(command_rx);

        registry.insert_peer(
            "node-b".to_string(),
            PeerHandle {
                command_tx,
                addr: "127.0.0.1:12345"
                    .parse()
                    .expect("test addr should be valid"),
                outbound: true,
                device_id: "node-b-device".to_string(),
                session_id: 1,
                connected_at_ms: 10,
            },
        );

        let result = registry.forward_file_decision(FileDecisionInput {
            peer_noob_id: "node-b".to_string(),
            transfer_id: 7,
            accept: false,
            reason: Some("reject".to_string()),
        });

        assert!(matches!(result, Err(ConnectionError::State(_))));
    }

    #[test]
    fn remove_peer_if_session_ignores_stale_disconnect() {
        let mut registry = PeerRegistry::new();
        let (command_tx_old, _command_rx_old) = mpsc::channel(1);
        let (command_tx_new, _command_rx_new) = mpsc::channel(1);

        registry.insert_peer(
            "node-b".to_string(),
            PeerHandle {
                command_tx: command_tx_old,
                addr: "127.0.0.1:10001"
                    .parse()
                    .expect("test addr should be valid"),
                outbound: false,
                device_id: "node-b-old".to_string(),
                session_id: 1,
                connected_at_ms: 1,
            },
        );
        registry.insert_peer(
            "node-b".to_string(),
            PeerHandle {
                command_tx: command_tx_new,
                addr: "127.0.0.1:10002"
                    .parse()
                    .expect("test addr should be valid"),
                outbound: true,
                device_id: "node-b-new".to_string(),
                session_id: 2,
                connected_at_ms: 2,
            },
        );

        assert!(!registry.remove_peer_if_session("node-b", 1));
        let snapshot = registry.snapshot();
        assert_eq!(snapshot.len(), 1);
        assert_eq!(
            snapshot[0].addr,
            "127.0.0.1:10002"
                .parse::<SocketAddr>()
                .expect("addr must parse")
        );
    }

    #[tokio::test]
    async fn send_text_only_dispatches_to_targets() {
        let mut registry = PeerRegistry::new();
        let mut receiver_b = insert_peer_for_test(&mut registry, "node-b");
        let mut receiver_c = insert_peer_for_test(&mut registry, "node-c");

        registry.send_text(SendTextRequest {
            event_id: "evt-1".to_string(),
            content: "hello".to_string(),
            targets: Some(vec![
                "node-c".to_string(),
                "missing-node".to_string(),
                "node-c".to_string(),
            ]),
        });

        let received = timeout(Duration::from_millis(100), receiver_c.recv())
            .await
            .expect("node-c should receive text")
            .expect("session command should exist");
        match received {
            SessionCommand::SendText(request) => {
                assert_eq!(request.event_id, "evt-1");
                assert_eq!(request.content, "hello");
                assert!(request.targets.is_none());
            }
            other => panic!("unexpected command: {other:?}"),
        }

        assert!(
            timeout(Duration::from_millis(100), receiver_b.recv())
                .await
                .is_err(),
            "node-b should not receive targeted text"
        );
    }

    #[tokio::test]
    async fn send_file_only_dispatches_to_targets() {
        let mut registry = PeerRegistry::new();
        let mut receiver_b = insert_peer_for_test(&mut registry, "node-b");
        let mut receiver_c = insert_peer_for_test(&mut registry, "node-c");

        let scheduled = registry
            .send_file(SendFileRequest {
                path: std::path::PathBuf::from("/tmp/demo.txt"),
                targets: Some(vec!["node-b".to_string()]),
            })
            .expect("send_file should succeed");
        assert_eq!(
            scheduled,
            vec![ScheduledTransfer {
                peer_noob_id: "node-b".to_string(),
                transfer_id: 1,
            }]
        );

        let received = timeout(Duration::from_millis(100), receiver_b.recv())
            .await
            .expect("node-b should receive file command")
            .expect("session command should exist");
        match received {
            SessionCommand::SendFile { transfer_id, path } => {
                assert_eq!(transfer_id, 1);
                assert_eq!(path, std::path::PathBuf::from("/tmp/demo.txt"));
            }
            other => panic!("unexpected command: {other:?}"),
        }

        assert!(
            timeout(Duration::from_millis(100), receiver_c.recv())
                .await
                .is_err(),
            "node-c should not receive targeted file"
        );
    }

    #[tokio::test]
    async fn cancel_transfer_routes_to_target_peer() {
        let mut registry = PeerRegistry::new();
        let mut receiver_b = insert_peer_for_test(&mut registry, "node-b");

        let cancel_task = tokio::spawn(async move {
            registry
                .cancel_transfer(CancelTransferRequest {
                    peer_noob_id: "node-b".to_string(),
                    transfer_id: 7,
                })
                .await
        });

        let received = timeout(Duration::from_millis(100), receiver_b.recv())
            .await
            .expect("node-b should receive cancel command")
            .expect("session command should exist");
        match received {
            SessionCommand::CancelTransfer { transfer_id, reply } => {
                assert_eq!(transfer_id, 7);
                reply.send(Ok(())).expect("session reply should send");
            }
            other => panic!("unexpected command: {other:?}"),
        }

        assert!(cancel_task.await.expect("task should join").is_ok());
    }
}
