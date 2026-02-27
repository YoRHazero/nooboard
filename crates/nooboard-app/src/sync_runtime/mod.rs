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
}

impl Default for SyncRuntime {
    fn default() -> Self {
        Self::new()
    }
}
