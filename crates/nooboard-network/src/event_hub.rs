use tokio::sync::broadcast;

use crate::{NetworkEvent, NetworkSubscription};

const EVENT_CHANNEL_CAPACITY: usize = 256;

#[derive(Clone)]
pub(crate) struct EventHub {
    sender: broadcast::Sender<NetworkEvent>,
}

impl EventHub {
    pub(crate) fn new() -> Self {
        let (sender, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        Self { sender }
    }

    pub(crate) fn subscribe(&self) -> NetworkSubscription {
        NetworkSubscription::new(self.sender.subscribe())
    }

    pub(crate) fn publish(&self, event: NetworkEvent) {
        let _ = self.sender.send(event);
    }

    pub(crate) fn publish_all<I>(&self, events: I)
    where
        I: IntoIterator<Item = NetworkEvent>,
    {
        for event in events {
            self.publish(event);
        }
    }
}
