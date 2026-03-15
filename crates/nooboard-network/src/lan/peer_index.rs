use std::collections::{BTreeMap, HashMap, HashSet};
use std::net::SocketAddr;

use crate::LanPeerInfo;
use crate::connection::policy::{DedupeDecision, dedupe_decision};

const LAN_BACKOFF_STEPS_MS: [u64; 5] = [5_000, 15_000, 30_000, 60_000, 60_000];

#[derive(Debug, Clone)]
pub(crate) struct LanServiceRecord {
    pub(crate) fullname: String,
    pub(crate) noob_id: String,
    pub(crate) device_id: String,
    pub(crate) addresses: Vec<SocketAddr>,
    pub(crate) last_seen_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LanConnectCandidate {
    pub(crate) noob_id: String,
    pub(crate) device_id: String,
    pub(crate) addr: SocketAddr,
}

#[derive(Debug, Clone, Default)]
struct LanPeerState {
    failure_count: u8,
    next_retry_at_ms: u64,
    next_addr_index: usize,
    connecting: bool,
}

#[derive(Debug, Default)]
pub(crate) struct LanPeerIndex {
    services: BTreeMap<String, LanServiceRecord>,
    peer_state: HashMap<String, LanPeerState>,
}

impl LanPeerIndex {
    pub(crate) fn apply_resolved(&mut self, record: LanServiceRecord) {
        self.services
            .insert(record.fullname.clone(), record.clone());
        self.peer_state.entry(record.noob_id).or_default();
    }

    pub(crate) fn apply_removed(&mut self, fullname: &str) -> bool {
        let Some(removed) = self.services.remove(fullname) else {
            return false;
        };

        if !self
            .services
            .values()
            .any(|service| service.noob_id == removed.noob_id)
        {
            self.peer_state.remove(&removed.noob_id);
        }
        true
    }

    pub(crate) fn clear(&mut self) {
        self.services.clear();
        self.peer_state.clear();
    }

    pub(crate) fn snapshot(&self, connected_noob_ids: &HashSet<String>) -> Vec<LanPeerInfo> {
        let mut merged: BTreeMap<&str, LanPeerInfo> = BTreeMap::new();

        for service in self.services.values() {
            let entry = merged
                .entry(&service.noob_id)
                .or_insert_with(|| LanPeerInfo {
                    noob_id: service.noob_id.clone(),
                    device_id: service.device_id.clone(),
                    addresses: Vec::new(),
                    last_seen_at_ms: service.last_seen_at_ms,
                    connected: connected_noob_ids.contains(&service.noob_id),
                });
            entry.device_id = service.device_id.clone();
            entry.last_seen_at_ms = entry.last_seen_at_ms.max(service.last_seen_at_ms);
            entry.connected = connected_noob_ids.contains(&service.noob_id);
            entry.addresses.extend(service.addresses.iter().copied());
        }

        let mut peers: Vec<_> = merged
            .into_values()
            .map(|mut peer| {
                peer.addresses.sort();
                peer.addresses.dedup();
                peer
            })
            .collect();
        peers.sort_by(|left, right| left.noob_id.cmp(&right.noob_id));
        peers
    }

    pub(crate) fn next_candidate(
        &self,
        local_noob_id: &str,
        connected_noob_ids: &HashSet<String>,
        now_ms: u64,
    ) -> Option<LanConnectCandidate> {
        for peer in self.snapshot(connected_noob_ids) {
            if connected_noob_ids.contains(&peer.noob_id) {
                continue;
            }
            if dedupe_decision(local_noob_id, &peer.noob_id) != DedupeDecision::ConnectOut {
                continue;
            }

            let state = self
                .peer_state
                .get(&peer.noob_id)
                .cloned()
                .unwrap_or_default();
            if state.connecting || now_ms < state.next_retry_at_ms || peer.addresses.is_empty() {
                continue;
            }

            let addr = peer.addresses[state.next_addr_index % peer.addresses.len()];
            return Some(LanConnectCandidate {
                noob_id: peer.noob_id,
                device_id: peer.device_id,
                addr,
            });
        }

        None
    }

    pub(crate) fn mark_connecting(&mut self, peer_noob_id: &str) {
        self.peer_state
            .entry(peer_noob_id.to_string())
            .or_default()
            .connecting = true;
    }

    pub(crate) fn note_connect_success(&mut self, peer_noob_id: &str) {
        if let Some(state) = self.peer_state.get_mut(peer_noob_id) {
            *state = LanPeerState::default();
        }
    }

    pub(crate) fn note_connect_finished(&mut self, peer_noob_id: &str) {
        if let Some(state) = self.peer_state.get_mut(peer_noob_id) {
            state.connecting = false;
        }
    }

    pub(crate) fn note_connect_failure(&mut self, peer_noob_id: &str, now_ms: u64) {
        let Some(peer) = self
            .snapshot(&HashSet::new())
            .into_iter()
            .find(|peer| peer.noob_id == peer_noob_id)
        else {
            return;
        };

        let state = self.peer_state.entry(peer_noob_id.to_string()).or_default();
        state.connecting = false;
        state.failure_count = state.failure_count.saturating_add(1);
        let backoff_index =
            usize::from(state.failure_count.saturating_sub(1)).min(LAN_BACKOFF_STEPS_MS.len() - 1);
        state.next_retry_at_ms = now_ms + LAN_BACKOFF_STEPS_MS[backoff_index];
        if !peer.addresses.is_empty() {
            state.next_addr_index = (state.next_addr_index + 1) % peer.addresses.len();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(fullname: &str, noob_id: &str, addr: &str, seen: u64) -> LanServiceRecord {
        LanServiceRecord {
            fullname: fullname.to_string(),
            noob_id: noob_id.to_string(),
            device_id: format!("device-{noob_id}"),
            addresses: vec![addr.parse().expect("addr")],
            last_seen_at_ms: seen,
        }
    }

    #[test]
    fn services_merge_by_noob_id_in_snapshot() {
        let mut index = LanPeerIndex::default();
        index.apply_resolved(record("one", "peer-a", "10.0.0.2:17890", 1));
        index.apply_resolved(record("two", "peer-a", "10.0.0.3:17890", 2));

        let peers = index.snapshot(&HashSet::new());
        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0].addresses.len(), 2);
        assert_eq!(peers[0].last_seen_at_ms, 2);
    }

    #[test]
    fn smaller_local_noob_id_connects_out() {
        let mut index = LanPeerIndex::default();
        index.apply_resolved(record("one", "peer-b", "10.0.0.2:17890", 1));

        let candidate = index
            .next_candidate("peer-a", &HashSet::new(), 0)
            .expect("candidate");
        assert_eq!(candidate.noob_id, "peer-b");
    }

    #[test]
    fn backoff_advances_after_failure() {
        let mut index = LanPeerIndex::default();
        index.apply_resolved(record("one", "peer-b", "10.0.0.2:17890", 1));
        index.note_connect_failure("peer-b", 100);

        assert!(
            index
                .next_candidate("peer-a", &HashSet::new(), 5_000)
                .is_none()
        );
        assert!(
            index
                .next_candidate("peer-a", &HashSet::new(), 5_100)
                .is_some()
        );
    }
}
