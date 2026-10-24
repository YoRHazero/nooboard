use crate::contracts::terminal::{RunId, TerminalConfigReference};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};
type Retained = BTreeMap<uuid::Uuid, (Option<RunId>, Vec<TerminalConfigReference>)>;
#[derive(Clone, Default)]
pub(crate) struct References(Arc<Mutex<Retained>>);
pub(crate) struct Reservation {
    refs: References,
    ticket: uuid::Uuid,
}
impl References {
    pub fn reserve(&self, run: Option<RunId>, keys: &[TerminalConfigReference]) -> Reservation {
        let ticket = uuid::Uuid::new_v4();
        let mut keys = keys.to_vec();
        keys.sort();
        keys.dedup();
        self.0.lock().unwrap().insert(ticket, (run, keys));
        Reservation {
            refs: self.clone(),
            ticket,
        }
    }
    pub fn uses(&self, key: TerminalConfigReference) -> Vec<Option<RunId>> {
        self.0
            .lock()
            .unwrap()
            .values()
            .filter(|(_, keys)| keys.contains(&key))
            .map(|(run, _)| *run)
            .collect()
    }
}
impl Drop for Reservation {
    fn drop(&mut self) {
        self.refs.0.lock().unwrap().remove(&self.ticket);
    }
}
