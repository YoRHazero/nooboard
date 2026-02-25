use std::collections::HashMap;
use std::net::SocketAddr;

use crate::discovery::DiscoveredPeer;

use super::policy::{DedupeDecision, dedupe_decision};

#[derive(Debug, Clone)]
pub(super) struct ConnectTarget {
    pub(super) addr: SocketAddr,
    pub(super) expected_node_id: Option<String>,
}

#[derive(Debug, Default)]
pub(super) struct CandidateRegistry {
    discovered_targets: HashMap<String, SocketAddr>,
}

impl CandidateRegistry {
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

    pub(super) fn apply_discovered_peer(
        &mut self,
        local_node_id: &str,
        peer: &DiscoveredPeer,
    ) -> DedupeDecision {
        let decision = dedupe_decision(local_node_id, &peer.node_id);
        match decision {
            DedupeDecision::ConnectOut => {
                self.discovered_targets.insert(peer.node_id.clone(), peer.addr);
            }
            DedupeDecision::WaitInbound => {
                self.discovered_targets.remove(&peer.node_id);
            }
            DedupeDecision::RejectConflict => {}
        }
        decision
    }
}
