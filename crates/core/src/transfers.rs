//! One activity per send operation, with independent outcomes for every destination.
use nooboard_network::MessageId;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, VecDeque},
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum DeliveryState {
    Queued,
    Sending,
    AwaitingReceipt,
    Applied,
    Rejected,
    Unconfirmed,
    Offline,
    Cancelled,
    Superseded,
    QueueFull,
}
impl DeliveryState {
    pub fn pending(self) -> bool {
        matches!(self, Self::Queued | Self::Sending | Self::AwaitingReceipt)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Delivery {
    pub noob_id: String,
    pub device_name: String,
    pub state: DeliveryState,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Transfer {
    pub id: MessageId,
    pub automatic: bool,
    pub bytes: usize,
    pub targets: Vec<Delivery>,
}
impl Transfer {
    pub fn pending(&self) -> bool {
        self.targets.iter().any(|d| d.state.pending())
    }
}
#[derive(Default)]
pub(crate) struct Transfers {
    entries: VecDeque<Transfer>,
    deadlines: HashMap<(MessageId, String), Instant>,
}
impl Transfers {
    pub fn available(&self) -> bool {
        self.entries.iter().filter(|e| e.pending()).count() < 1024
    }
    pub fn insert(&mut self, transfer: Transfer) {
        self.entries.push_front(transfer);
        self.prune();
    }
    fn prune(&mut self) {
        let mut completed = 0;
        self.entries.retain(|t| {
            if t.pending() {
                true
            } else {
                completed += 1;
                completed <= 30
            }
        });
    }
    pub fn snapshot(&self) -> Vec<Transfer> {
        self.entries.iter().cloned().collect()
    }
    pub fn update(&mut self, id: &MessageId, peer: &str, next: DeliveryState) -> Option<Transfer> {
        let transfer = self.entries.iter_mut().find(|t| t.id == *id)?;
        let target = transfer.targets.iter_mut().find(|d| d.noob_id == peer)?;
        let late_receipt = target.state == DeliveryState::Unconfirmed
            && matches!(next, DeliveryState::Applied | DeliveryState::Rejected);
        if (!target.state.pending() && !late_receipt) || target.state == next {
            return None;
        }
        if next == DeliveryState::Sending && target.state != DeliveryState::Queued {
            return None;
        }
        if next == DeliveryState::AwaitingReceipt && target.state != DeliveryState::Sending {
            return None;
        }
        if matches!(next, DeliveryState::Applied | DeliveryState::Rejected)
            && target.state == DeliveryState::Queued
        {
            return None;
        }
        target.state = next;
        let key = (id.clone(), peer.to_owned());
        if next == DeliveryState::Sending {
            self.deadlines
                .insert(key.clone(), Instant::now() + Duration::from_secs(30));
        }
        if !next.pending() {
            self.deadlines.remove(&key);
        }
        let result = transfer.clone();
        self.prune();
        Some(result)
    }
    pub fn disconnect(&mut self, peer: &str) -> Vec<Transfer> {
        let pending: Vec<_> = self
            .entries
            .iter()
            .flat_map(|t| {
                t.targets
                    .iter()
                    .filter(|d| d.noob_id == peer && d.state.pending())
                    .map(|d| (t.id.clone(), d.state))
            })
            .collect();
        pending
            .into_iter()
            .filter_map(|(id, state)| {
                self.update(
                    &id,
                    peer,
                    if state == DeliveryState::Queued {
                        DeliveryState::Cancelled
                    } else {
                        DeliveryState::Unconfirmed
                    },
                )
            })
            .collect()
    }
    pub fn expire(&mut self, now: Instant) -> Vec<Transfer> {
        let expired: Vec<_> = self
            .deadlines
            .iter()
            .filter(|(_, time)| **time <= now)
            .map(|(key, _)| key.clone())
            .collect();
        expired
            .into_iter()
            .filter_map(|(id, peer)| self.update(&id, &peer, DeliveryState::Unconfirmed))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn transfer() -> Transfer {
        Transfer {
            id: MessageId {
                session: "a".repeat(32),
                sequence: 1,
            },
            automatic: false,
            bytes: 4,
            targets: ["a", "b"]
                .into_iter()
                .map(|id| Delivery {
                    noob_id: id.into(),
                    device_name: "同名".into(),
                    state: DeliveryState::Queued,
                })
                .collect(),
        }
    }
    #[test]
    fn early_receipt_and_wrong_peer_cannot_complete_a_queued_delivery() {
        let mut book = Transfers::default();
        let t = transfer();
        book.insert(t.clone());
        assert!(book.update(&t.id, "a", DeliveryState::Applied).is_none());
        assert!(
            book.update(&t.id, "stranger", DeliveryState::Applied)
                .is_none()
        );
        book.update(&t.id, "a", DeliveryState::Sending).unwrap();
        book.update(&t.id, "a", DeliveryState::Applied).unwrap();
        assert!(
            book.update(&t.id, "a", DeliveryState::AwaitingReceipt)
                .is_none()
        );
        assert_eq!(book.snapshot()[0].targets[1].state, DeliveryState::Queued);
    }
    #[test]
    fn missing_receipt_expires_independently_and_pending_batches_survive_retention() {
        let mut book = Transfers::default();
        let t = transfer();
        book.insert(t.clone());
        book.update(&t.id, "a", DeliveryState::Sending).unwrap();
        for sequence in 2..100 {
            let mut done = transfer();
            done.id.sequence = sequence;
            for d in &mut done.targets {
                d.state = DeliveryState::Applied;
            }
            book.insert(done);
        }
        assert_eq!(book.snapshot().len(), 31);
        let expired = book.expire(Instant::now() + Duration::from_secs(31));
        assert_eq!(expired.len(), 1);
        assert_eq!(expired[0].targets[0].state, DeliveryState::Unconfirmed);
        assert_eq!(expired[0].targets[1].state, DeliveryState::Queued);
        // A late authenticated receipt can resolve uncertainty without retransmitting anything.
        assert_eq!(
            book.update(&t.id, "a", DeliveryState::Applied)
                .unwrap()
                .targets[0]
                .state,
            DeliveryState::Applied
        );
        assert_eq!(
            book.disconnect("b")[0].targets[1].state,
            DeliveryState::Cancelled
        );
    }
}
