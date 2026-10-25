//! Bounded WebView delivery. At most one output frame per subscription awaits an ACK.
//! Core retains the replay window and reports gaps if the view cannot keep up.
use super::{
    contracts::TerminalOutputFrame,
    errors::{self, UiError},
};
use nooboard_core::terminal::{CancellationToken, TerminalOutputSubscription};
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};
use tauri::ipc::Channel;
use tokio::sync::watch;
struct Slot {
    serial: u64,
    token: CancellationToken,
    ack: watch::Sender<u64>,
    expected: Arc<AtomicU64>,
}
#[derive(Default)]
struct State {
    owner: Option<String>,
    sequence: u64,
    slots: HashMap<String, Slot>,
}
#[derive(Clone, Default)]
pub struct TerminalIo(Arc<Mutex<State>>);
pub struct OutputLease {
    pub token: CancellationToken,
    serial: u64,
    ack: watch::Receiver<u64>,
    expected: Arc<AtomicU64>,
}
impl TerminalIo {
    pub fn activate(&self, owner: String) {
        let mut state = self.0.lock().unwrap();
        for (_, slot) in state.slots.drain() {
            slot.token.cancel();
        }
        state.owner = Some(owner);
    }
    pub fn deactivate(&self, owner: &str) {
        let mut state = self.0.lock().unwrap();
        if state.owner.as_deref() == Some(owner) {
            for (_, slot) in state.slots.drain() {
                slot.token.cancel();
            }
            state.owner = None;
        }
    }
    pub fn clear(&self) {
        let mut state = self.0.lock().unwrap();
        for (_, slot) in state.slots.drain() {
            slot.token.cancel();
        }
        state.owner = None;
    }
    pub fn begin(&self, owner: &str, id: &str, capacity: usize) -> Result<OutputLease, UiError> {
        if id.is_empty() || id.len() > 128 {
            return Err(errors::ui("subscription"));
        }
        let mut state = self.0.lock().unwrap();
        if state.owner.as_deref() != Some(owner) {
            return Err(errors::ui("stopped"));
        }
        if state.slots.len() >= capacity {
            return Err(errors::ui("busy"));
        }
        if state.slots.contains_key(id) {
            return Err(errors::ui("subscription"));
        }
        let token = CancellationToken::new();
        state.sequence = state.sequence.wrapping_add(1);
        let serial = state.sequence;
        let (ack, receiver) = watch::channel(0);
        let expected = Arc::new(AtomicU64::new(0));
        state.slots.insert(
            id.into(),
            Slot {
                serial,
                token: token.clone(),
                ack,
                expected: expected.clone(),
            },
        );
        Ok(OutputLease {
            token,
            serial,
            ack: receiver,
            expected,
        })
    }
    pub fn acknowledge(&self, owner: &str, id: &str, sequence: u64) {
        let state = self.0.lock().unwrap();
        if state.owner.as_deref() == Some(owner)
            && let Some(slot) = state.slots.get(id)
            && slot.expected.load(Ordering::Acquire) == sequence
        {
            slot.ack.send_replace(sequence);
        }
    }
    pub fn detach(&self, owner: &str, id: &str) {
        let mut state = self.0.lock().unwrap();
        if state.owner.as_deref() == Some(owner)
            && let Some(slot) = state.slots.remove(id)
        {
            slot.token.cancel();
        }
    }
    fn finish(&self, owner: &str, id: &str, serial: u64) {
        let mut state = self.0.lock().unwrap();
        if state.owner.as_deref() == Some(owner)
            && state
                .slots
                .get(id)
                .is_some_and(|slot| slot.serial == serial)
        {
            state.slots.remove(id);
        }
    }
    pub fn release(&self, owner: &str, id: &str, lease: OutputLease) {
        self.finish(owner, id, lease.serial);
    }
    pub fn pump(
        &self,
        owner: String,
        id: String,
        mut lease: OutputLease,
        mut output: TerminalOutputSubscription,
        channel: Channel<TerminalOutputFrame>,
    ) {
        let registry = self.clone();
        tokio::spawn(async move {
            let mut sequence = 1u64;
            let mut frame = TerminalOutputFrame::Data {
                sequence: sequence.to_string(),
                offset: output.initial_output.offset.to_string(),
                bytes: output.initial_output.data,
            };
            loop {
                if lease.token.is_cancelled() {
                    break;
                }
                lease.expected.store(sequence, Ordering::Release);
                if channel.send(frame).is_err() {
                    break;
                }
                let acknowledged = tokio::select! {_=lease.token.cancelled()=>false,result=lease.ack.wait_for(|value|*value==sequence)=>result.is_ok()};
                if !acknowledged {
                    break;
                }
                sequence = sequence.wrapping_add(1);
                let next = tokio::select! {_=lease.token.cancelled()=>break,next=output.stream.next()=>next};
                frame = match next {
                    Ok(Some(chunk)) => TerminalOutputFrame::Data {
                        sequence: sequence.to_string(),
                        offset: chunk.offset.to_string(),
                        bytes: chunk.data,
                    },
                    Err(nooboard_core::terminal::TerminalOutputError::Lagged {
                        available_offset,
                        ..
                    }) => TerminalOutputFrame::Gap {
                        sequence: sequence.to_string(),
                        offset: available_offset.to_string(),
                    },
                    Ok(None) => {
                        let _ = channel.send(TerminalOutputFrame::Closed);
                        break;
                    }
                };
            }
            registry.finish(&owner, &id, lease.serial);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn replacing_view_cancels_old_streams_but_stale_disconnect_keeps_new_owner() {
        let io = TerminalIo::default();
        io.activate("old".into());
        let old = io.begin("old", "output", 1).unwrap();
        assert!(io.begin("old", "another", 1).is_err());
        io.activate("new".into());
        assert!(old.token.is_cancelled());
        let new = io.begin("new", "output", 1).unwrap();
        io.deactivate("old");
        assert!(!new.token.is_cancelled());
        assert!(io.begin("old", "other", 2).is_err());
        io.clear();
        assert!(new.token.is_cancelled());
    }
    #[test]
    fn only_expected_ack_is_accepted_and_stale_finish_cannot_remove_reused_slot() {
        let io = TerminalIo::default();
        io.activate("view".into());
        let old = io.begin("view", "output", 1).unwrap();
        old.expected.store(7, Ordering::Release);
        io.acknowledge("other", "output", 7);
        io.acknowledge("view", "output", 6);
        assert_eq!(*old.ack.borrow(), 0);
        io.acknowledge("view", "output", 7);
        assert_eq!(*old.ack.borrow(), 7);
        io.detach("view", "output");
        assert!(old.token.is_cancelled());
        let new = io.begin("view", "output", 1).unwrap();
        io.release("view", "output", old);
        assert!(io.begin("view", "another", 1).is_err());
        io.detach("view", "output");
        assert!(new.token.is_cancelled());
    }
}
