use std::sync::Mutex;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Idle,
    Asking,
    Closing,
    Finished,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ticket(u64);
struct State {
    phase: Phase,
    generation: u64,
}
pub struct ExitControl(Mutex<State>);
impl Default for ExitControl {
    fn default() -> Self {
        Self(Mutex::new(State {
            phase: Phase::Idle,
            generation: 0,
        }))
    }
}
impl ExitControl {
    pub fn phase(&self) -> Phase {
        self.0.lock().unwrap().phase
    }
    pub fn exiting(&self) -> bool {
        matches!(self.phase(), Phase::Closing | Phase::Finished)
    }
    pub fn finished(&self) -> bool {
        self.phase() == Phase::Finished
    }
    pub fn ask(&self) -> Option<Ticket> {
        let mut state = self.0.lock().unwrap();
        if state.phase != Phase::Idle {
            return None;
        }
        state.generation = state.generation.wrapping_add(1);
        state.phase = Phase::Asking;
        Some(Ticket(state.generation))
    }
    pub fn is_asking(&self, ticket: Ticket) -> bool {
        let state = self.0.lock().unwrap();
        state.phase == Phase::Asking && state.generation == ticket.0
    }
    pub fn cancel(&self, ticket: Ticket) {
        let mut state = self.0.lock().unwrap();
        if state.phase == Phase::Asking && state.generation == ticket.0 {
            state.phase = Phase::Idle;
        }
    }
    pub fn begin(&self, confirmation: Option<Ticket>) -> Option<Ticket> {
        let mut state = self.0.lock().unwrap();
        if matches!(state.phase, Phase::Closing | Phase::Finished) {
            return None;
        }
        if let Some(ticket) = confirmation
            && (state.phase != Phase::Asking || state.generation != ticket.0)
        {
            return None;
        }
        state.generation = state.generation.wrapping_add(1);
        state.phase = Phase::Closing;
        Some(Ticket(state.generation))
    }
    pub fn complete(&self, ticket: Ticket, success: bool) -> bool {
        let mut state = self.0.lock().unwrap();
        if state.phase != Phase::Closing || state.generation != ticket.0 {
            return false;
        }
        state.phase = if success {
            Phase::Finished
        } else {
            Phase::Idle
        };
        true
    }
}
pub fn should_hide(supported: bool, available: bool, enabled: bool, exiting: bool) -> bool {
    supported && available && enabled && !exiting
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unsupported_or_failed_tray_never_hides_the_window() {
        assert!(should_hide(true, true, true, false));
        for (supported, available, enabled, exiting) in [
            (false, true, true, false),
            (true, false, true, false),
            (true, true, false, false),
            (true, true, true, true),
        ] {
            assert!(!should_hide(supported, available, enabled, exiting));
        }
    }
}
