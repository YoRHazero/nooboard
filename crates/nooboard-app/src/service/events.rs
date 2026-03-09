use tokio::sync::broadcast;

use crate::service::types::{AppEvent, EventSubscription};

const EVENT_CHANNEL_CAPACITY: usize = 256;

#[derive(Clone)]
pub(crate) struct EventHub {
    sender: broadcast::Sender<AppEvent>,
}

impl EventHub {
    pub(crate) fn new() -> Self {
        let (sender, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        Self { sender }
    }

    pub(crate) fn subscribe(&self) -> EventSubscription {
        EventSubscription::new(self.sender.subscribe())
    }

    pub(crate) fn publish(&self, event: AppEvent) {
        let _ = self.sender.send(event);
    }
}
