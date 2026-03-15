use tokio::sync::broadcast;

use crate::{NetworkEvent, NetworkSubscription};

const EVENT_CHANNEL_CAPACITY: usize = 256;

#[cfg(test)]
type PublishHook = std::sync::Arc<dyn Fn(&NetworkEvent) + Send + Sync>;

#[derive(Clone)]
pub(crate) struct EventHub {
    sender: broadcast::Sender<NetworkEvent>,
    #[cfg(test)]
    before_send_hook: std::sync::Arc<std::sync::Mutex<Option<PublishHook>>>,
}

impl EventHub {
    pub(crate) fn new() -> Self {
        let (sender, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        Self {
            sender,
            #[cfg(test)]
            before_send_hook: std::sync::Arc::new(std::sync::Mutex::new(None)),
        }
    }

    pub(crate) fn subscribe(&self) -> NetworkSubscription {
        NetworkSubscription::new(self.sender.subscribe())
    }

    pub(crate) fn publish(&self, event: NetworkEvent) {
        #[cfg(test)]
        {
            let hook = self
                .before_send_hook
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .clone();
            if let Some(hook) = hook {
                hook(&event);
            }
        }
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

    #[cfg(test)]
    pub(crate) fn set_before_send_hook(&self, hook: PublishHook) {
        *self
            .before_send_hook
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(hook);
    }
}
