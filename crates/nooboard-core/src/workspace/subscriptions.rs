use tokio::sync::{broadcast, watch};

use crate::types::{EventSubscription, StateSubscription, WorkspaceEvent, WorkspaceSnapshot};

const EVENT_CHANNEL_CAPACITY: usize = 256;

#[derive(Clone)]
pub(crate) struct StateHub {
    sender: watch::Sender<WorkspaceSnapshot>,
}

impl StateHub {
    pub(crate) fn new(initial: WorkspaceSnapshot) -> Self {
        let (sender, _) = watch::channel(initial);
        Self { sender }
    }

    pub(crate) fn subscribe(&self) -> StateSubscription {
        StateSubscription::new(self.sender.subscribe())
    }

    pub(crate) fn publish(&self, snapshot: WorkspaceSnapshot) {
        let _ = self.sender.send(snapshot);
    }
}

#[derive(Clone)]
pub(crate) struct EventHub {
    sender: broadcast::Sender<WorkspaceEvent>,
}

impl EventHub {
    pub(crate) fn new() -> Self {
        let (sender, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        Self { sender }
    }

    pub(crate) fn subscribe(&self) -> EventSubscription {
        EventSubscription::new(self.sender.subscribe())
    }

    pub(crate) fn publish(&self, event: WorkspaceEvent) {
        let _ = self.sender.send(event);
    }
}
