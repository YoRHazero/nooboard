use tokio::sync::watch;

use crate::service::types::{AppState, StateSubscription};

#[derive(Clone)]
pub(crate) struct StateHub {
    sender: watch::Sender<AppState>,
}

impl StateHub {
    pub(crate) fn new(initial: AppState) -> Self {
        let (sender, _) = watch::channel(initial);
        Self { sender }
    }

    pub(crate) fn subscribe(&self) -> StateSubscription {
        StateSubscription::new(self.sender.subscribe())
    }

    pub(crate) fn publish(&self, state: AppState) {
        let _ = self.sender.send(state);
    }
}
