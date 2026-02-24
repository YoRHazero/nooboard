use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::path::PathBuf;

use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_rustls::TlsStream;
use tokio_util::codec::{Framed, LengthDelimitedCodec};

use crate::connection::actor::ConnectionCommand;
use crate::discovery::{DedupeDecision, DiscoveredPeer, dedupe_decision};

use super::types::FileDecisionInput;

#[derive(Debug)]
pub(super) struct PeerHandle {
    pub(super) command_tx: mpsc::Sender<ConnectionCommand>,
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
    ConnectAttemptFinished {
        addr: SocketAddr,
    },
    PeerDisconnected {
        peer_node_id: String,
    },
    DiscoveredPeer(DiscoveredPeer),
}

#[derive(Debug, Clone)]
pub(super) struct ConnectTarget {
    pub(super) addr: SocketAddr,
    pub(super) expected_node_id: Option<String>,
}

#[derive(Debug, Default)]
pub(super) struct PeerRegistry {
    peers: HashMap<String, PeerHandle>,
    connecting_addrs: HashSet<SocketAddr>,
    discovered_targets: HashMap<String, SocketAddr>,
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
        let mut targets = Vec::new();

        for addr in manual_peers {
            targets.push(ConnectTarget {
                addr: *addr,
                expected_node_id: None,
            });
        }

        for (node_id, addr) in &self.discovered_targets {
            targets.push(ConnectTarget {
                addr: *addr,
                expected_node_id: Some(node_id.clone()),
            });
        }

        targets
    }

    pub(super) fn should_skip_target(&self, addr: &SocketAddr) -> bool {
        self.connecting_addrs.contains(addr) || self.peers.values().any(|peer| peer.addr == *addr)
    }

    pub(super) async fn broadcast_text(&self, text: String) {
        for peer in self.peers.values() {
            let _ = peer
                .command_tx
                .send(ConnectionCommand::SendText(text.clone()))
                .await;
        }
    }

    pub(super) async fn broadcast_file(&self, path: PathBuf) {
        for peer in self.peers.values() {
            let _ = peer
                .command_tx
                .send(ConnectionCommand::SendFile(path.clone()))
                .await;
        }
    }

    pub(super) async fn forward_file_decision(&self, decision: FileDecisionInput) -> bool {
        if let Some(peer) = self.peers.get(&decision.peer_node_id) {
            let _ = peer
                .command_tx
                .send(ConnectionCommand::FileDecision {
                    transfer_id: decision.transfer_id,
                    accept: decision.accept,
                    reason: decision.reason,
                })
                .await;
            true
        } else {
            false
        }
    }

    pub(super) async fn disconnect_peer(&mut self, peer_node_id: &str) -> Option<SocketAddr> {
        let peer = self.peers.remove(peer_node_id)?;
        let addr = peer.addr;
        let _ = peer.command_tx.send(ConnectionCommand::Shutdown).await;
        Some(addr)
    }

    pub(super) async fn shutdown_all(&self) {
        for peer in self.peers.values() {
            let _ = peer.command_tx.send(ConnectionCommand::Shutdown).await;
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
    ) -> Option<mpsc::Sender<ConnectionCommand>> {
        self.peers
            .get(peer_node_id)
            .map(|peer| peer.command_tx.clone())
    }

    pub(super) fn apply_discovered_peer(
        &mut self,
        local_node_id: &str,
        peer: &DiscoveredPeer,
    ) -> DedupeDecision {
        let decision = dedupe_decision(local_node_id, &peer.node_id);
        match decision {
            DedupeDecision::ConnectOut => {
                self.discovered_targets
                    .insert(peer.node_id.clone(), peer.addr);
            }
            DedupeDecision::WaitInbound => {
                self.discovered_targets.remove(&peer.node_id);
            }
            DedupeDecision::RejectConflict => {}
        }
        decision
    }
}
