use std::collections::{HashMap, hash_map::Entry};
use std::net::SocketAddr;

use crate::discovery::DiscoveredPeer;
use crate::discovery::sort_socket_addrs_by_preference;

use super::policy::{DedupeDecision, dedupe_decision};

#[derive(Debug, Clone)]
pub(super) struct ConnectTarget {
    pub(super) addr: SocketAddr,
    pub(super) expected_noob_id: Option<String>,
}

#[derive(Debug, Default)]
pub(super) struct CandidateRegistry {
    discovered_targets: HashMap<String, DiscoveredTargetSet>,
}

#[derive(Debug, Clone)]
struct DiscoveredTargetSet {
    addrs: Vec<SocketAddr>,
    next_addr_idx: usize,
}

impl DiscoveredTargetSet {
    fn new(addrs: Vec<SocketAddr>) -> Option<Self> {
        if addrs.is_empty() {
            None
        } else {
            Some(Self {
                addrs,
                next_addr_idx: 0,
            })
        }
    }

    fn current_addr(&self) -> Option<SocketAddr> {
        if self.addrs.is_empty() {
            return None;
        }

        self.addrs
            .get(self.next_addr_idx % self.addrs.len())
            .copied()
    }

    fn replace_addrs(&mut self, mut addrs: Vec<SocketAddr>) {
        sort_socket_addrs_by_preference(&mut addrs);
        let current_addr = self.current_addr();
        self.next_addr_idx = current_addr
            .and_then(|addr| addrs.iter().position(|candidate| *candidate == addr))
            .unwrap_or(0);
        self.addrs = addrs;
    }

    fn advance_past(&mut self, addr: &SocketAddr) -> bool {
        if self.addrs.len() <= 1 || self.current_addr().as_ref() != Some(addr) {
            return false;
        }

        self.next_addr_idx = (self.next_addr_idx + 1) % self.addrs.len();
        true
    }
}

impl CandidateRegistry {
    pub(super) fn connect_targets(&self, manual_peers: &[SocketAddr]) -> Vec<ConnectTarget> {
        let mut targets = Vec::new();

        for addr in manual_peers {
            targets.push(ConnectTarget {
                addr: *addr,
                expected_noob_id: None,
            });
        }

        let mut discovered: Vec<_> = self.discovered_targets.iter().collect();
        discovered.sort_unstable_by(|(left, _), (right, _)| left.cmp(right));
        for (noob_id, target_set) in discovered {
            if let Some(addr) = target_set.current_addr() {
                targets.push(ConnectTarget {
                    addr,
                    expected_noob_id: Some(noob_id.clone()),
                });
            }
        }

        targets
    }

    pub(super) fn apply_discovered_peer(
        &mut self,
        local_noob_id: &str,
        peer: &DiscoveredPeer,
    ) -> DedupeDecision {
        let decision = dedupe_decision(local_noob_id, &peer.noob_id);
        match decision {
            DedupeDecision::ConnectOut => {
                let mut addrs = peer.addrs.clone();
                sort_socket_addrs_by_preference(&mut addrs);
                if addrs.is_empty() {
                    self.discovered_targets.remove(&peer.noob_id);
                    return decision;
                }

                match self.discovered_targets.entry(peer.noob_id.clone()) {
                    Entry::Occupied(mut entry) => entry.get_mut().replace_addrs(addrs),
                    Entry::Vacant(entry) => {
                        if let Some(target_set) = DiscoveredTargetSet::new(addrs) {
                            entry.insert(target_set);
                        }
                    }
                }
            }
            DedupeDecision::WaitInbound => {
                self.discovered_targets.remove(&peer.noob_id);
            }
            DedupeDecision::RejectConflict => {}
        }
        decision
    }

    pub(super) fn note_connect_failure(&mut self, addr: &SocketAddr) {
        for target_set in self.discovered_targets.values_mut() {
            if target_set.advance_past(addr) {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV6};

    use super::*;

    #[test]
    fn discovered_targets_keep_sorted_preferred_address() {
        let mut registry = CandidateRegistry::default();
        registry.apply_discovered_peer(
            "node-a",
            &DiscoveredPeer {
                noob_id: "node-b".to_string(),
                addrs: vec![
                    SocketAddr::V6(SocketAddrV6::new(
                        Ipv6Addr::new(0xfe80, 0, 0, 0, 0, 0, 0, 1),
                        17890,
                        0,
                        3,
                    )),
                    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 20)), 17890),
                    SocketAddr::V6(SocketAddrV6::new(
                        Ipv6Addr::new(0xfd00, 0, 0, 0, 0, 0, 0, 1),
                        17890,
                        0,
                        0,
                    )),
                ],
            },
        );

        let targets = registry.connect_targets(&[]);
        assert_eq!(targets.len(), 1);
        assert_eq!(
            targets[0].addr,
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 20)), 17890)
        );
    }

    #[test]
    fn connect_failure_rotates_to_next_discovered_address() {
        let mut registry = CandidateRegistry::default();
        let preferred = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 20)), 17890);
        let fallback = SocketAddr::V6(SocketAddrV6::new(
            Ipv6Addr::new(0xfd00, 0, 0, 0, 0, 0, 0, 1),
            17890,
            0,
            0,
        ));
        registry.apply_discovered_peer(
            "node-a",
            &DiscoveredPeer {
                noob_id: "node-b".to_string(),
                addrs: vec![fallback, preferred],
            },
        );

        registry.note_connect_failure(&preferred);

        let targets = registry.connect_targets(&[]);
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].addr, fallback);
    }

    #[test]
    fn refreshed_addresses_preserve_current_target_when_still_available() {
        let mut registry = CandidateRegistry::default();
        let preferred = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 20)), 17890);
        let fallback = SocketAddr::V6(SocketAddrV6::new(
            Ipv6Addr::new(0xfd00, 0, 0, 0, 0, 0, 0, 1),
            17890,
            0,
            0,
        ));
        registry.apply_discovered_peer(
            "node-a",
            &DiscoveredPeer {
                noob_id: "node-b".to_string(),
                addrs: vec![preferred, fallback],
            },
        );
        registry.note_connect_failure(&preferred);

        registry.apply_discovered_peer(
            "node-a",
            &DiscoveredPeer {
                noob_id: "node-b".to_string(),
                addrs: vec![preferred, fallback],
            },
        );

        let targets = registry.connect_targets(&[]);
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].addr, fallback);
    }
}
