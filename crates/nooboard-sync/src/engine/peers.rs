use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::path::PathBuf;

use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_rustls::TlsStream;
use tokio_util::codec::{Framed, LengthDelimitedCodec};

use crate::error::{ConnectionError, SyncError};
use crate::session::actor::SessionCommand;
use crate::discovery::DiscoveredPeer;

use super::candidates::{CandidateRegistry, ConnectTarget};
use super::policy::DedupeDecision;
use super::types::FileDecisionInput;

#[derive(Debug)]
pub(super) struct PeerHandle {
    pub(super) command_tx: mpsc::Sender<SessionCommand>,
    pub(super) addr: SocketAddr,
    pub(super) outbound: bool,
}

#[derive(Debug)]
pub(super) enum EngineControl {
    Connected {
        peer_node_id: String,
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
        peer_node_id: String,
        error: ConnectionError,
    },
    PeerDisconnected {
        peer_node_id: String,
    },
    DiscoveredPeer(DiscoveredPeer),
}

#[derive(Debug, Default)]
pub(super) struct PeerRegistry {
    peers: HashMap<String, PeerHandle>,
    connecting_addrs: HashSet<SocketAddr>,
    candidates: CandidateRegistry,
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

    pub(super) async fn broadcast_text(&self, text: String) {
        for peer in self.peers.values() {
            let _ = peer
                .command_tx
                .send(SessionCommand::SendText(text.clone()))
                .await;
        }
    }

    pub(super) async fn broadcast_file(&self, path: PathBuf) {
        for peer in self.peers.values() {
            let _ = peer
                .command_tx
                .send(SessionCommand::SendFile(path.clone()))
                .await;
        }
    }

    pub(super) async fn forward_file_decision(
        &self,
        decision: FileDecisionInput,
    ) -> Result<(), ConnectionError> {
        if let Some(peer) = self.peers.get(&decision.peer_node_id) {
            peer
                .command_tx
                .send(SessionCommand::FileDecision {
                    transfer_id: decision.transfer_id,
                    accept: decision.accept,
                    reason: decision.reason,
                })
                .await
                .map_err(|error| {
                    ConnectionError::State(format!(
                        "failed to forward FileDecision to peer {} transfer {}: {}",
                        decision.peer_node_id, decision.transfer_id, error
                    ))
                })?;
            Ok(())
        } else {
            Err(ConnectionError::State(format!(
                "peer {} is not connected",
                decision.peer_node_id
            )))
        }
    }

    pub(super) async fn disconnect_peer(&mut self, peer_node_id: &str) -> Option<SocketAddr> {
        let peer = self.peers.remove(peer_node_id)?;
        let addr = peer.addr;
        let _ = peer.command_tx.send(SessionCommand::Shutdown).await;
        Some(addr)
    }

    pub(super) async fn shutdown_all(&self) {
        for peer in self.peers.values() {
            let _ = peer.command_tx.send(SessionCommand::Shutdown).await;
        }
    }

    pub(super) fn remove_peer(&mut self, peer_node_id: &str) {
        self.peers.remove(peer_node_id);
    }

    pub(super) fn insert_peer(&mut self, peer_node_id: String, handle: PeerHandle) {
        self.peers.insert(peer_node_id, handle);
    }

    pub(super) fn peer_outbound(&self, peer_node_id: &str) -> Option<bool> {
        self.peers.get(peer_node_id).map(|peer| peer.outbound)
    }

    pub(super) fn peer_command_tx(
        &self,
        peer_node_id: &str,
    ) -> Option<mpsc::Sender<SessionCommand>> {
        self.peers
            .get(peer_node_id)
            .map(|peer| peer.command_tx.clone())
    }

    pub(super) fn apply_discovered_peer(
        &mut self,
        local_node_id: &str,
        peer: &DiscoveredPeer,
    ) -> DedupeDecision {
        self.candidates.apply_discovered_peer(local_node_id, peer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn forward_file_decision_returns_error_when_peer_missing() {
        let registry = PeerRegistry::new();
        let result = registry
            .forward_file_decision(FileDecisionInput {
                peer_node_id: "missing-peer".to_string(),
                transfer_id: 1,
                accept: true,
                reason: None,
            })
            .await;

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
            },
        );

        let result = registry
            .forward_file_decision(FileDecisionInput {
                peer_node_id: "node-b".to_string(),
                transfer_id: 7,
                accept: false,
                reason: Some("reject".to_string()),
            })
            .await;

        assert!(matches!(result, Err(ConnectionError::State(_))));
    }
}
