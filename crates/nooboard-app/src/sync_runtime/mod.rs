mod bridge;
mod commands;
mod lifecycle;
mod state;
mod subscriptions;

use state::RuntimeState;

pub struct SyncRuntime {
    state: RuntimeState,
}

impl SyncRuntime {
    pub fn new() -> Self {
        Self {
            state: RuntimeState::new(),
        }
    }

    pub fn has_engine(&self) -> bool {
        self.state.engine.is_some()
    }
}

impl Default for SyncRuntime {
    fn default() -> Self {
        Self::new()
    }
}
