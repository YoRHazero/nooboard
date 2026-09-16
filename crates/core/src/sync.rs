//! Bounded duplicate detection. New messages apply in local arrival order.
use nooboard_network::MessageId;
use std::collections::{HashMap, VecDeque};
#[derive(Default)]
pub(crate) struct Received {
    order: VecDeque<(String, MessageId)>,
    outcomes: HashMap<(String, MessageId), bool>,
}
impl Received {
    pub fn outcome(&self, peer: &str, id: &MessageId) -> Option<bool> {
        self.outcomes.get(&(peer.to_owned(), id.clone())).copied()
    }
    pub fn record(&mut self, peer: &str, id: &MessageId, applied: bool) {
        let key = (peer.to_owned(), id.clone());
        if self.outcomes.insert(key.clone(), applied).is_none() {
            self.order.push_back(key);
        }
        while self.order.len() > 4096 {
            if let Some(key) = self.order.pop_front() {
                self.outcomes.remove(&key);
            }
        }
    }
    pub fn forget(&mut self, peer: &str) {
        self.order.retain(|(p, _)| p != peer);
        self.outcomes.retain(|(p, _), _| p != peer);
    }
}
