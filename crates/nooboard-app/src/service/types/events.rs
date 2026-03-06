use std::net::SocketAddr;

use tokio::sync::broadcast;

use super::{EventId, NoobId, TransferUpdate};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventStream {
    Sync,
    Transfer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncEvent {
    TextReceived {
        event_id: EventId,
        content: String,
        noob_id: NoobId,
        device_id: String,
    },
    FileDecisionRequired {
        peer_noob_id: NoobId,
        transfer_id: u32,
        file_name: String,
        file_size: u64,
        total_chunks: u32,
    },
    ConnectionError {
        peer_noob_id: Option<NoobId>,
        addr: Option<SocketAddr>,
        error: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppEvent {
    Sync(SyncEvent),
    Transfer(TransferUpdate),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubscriptionCloseReason {
    EngineStopped,
    Rebinding { next_session_id: u64 },
    UpstreamClosed { stream: EventStream },
    Fatal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubscriptionLifecycle {
    Opened {
        session_id: u64,
    },
    Rebinding {
        from_session_id: u64,
        to_session_id: u64,
    },
    Lagged {
        session_id: u64,
        stream: EventStream,
        dropped: u64,
    },
    RecoverableError {
        session_id: u64,
        stream: EventStream,
        error: String,
    },
    Fatal {
        session_id: u64,
        error: String,
    },
    Closed {
        session_id: u64,
        reason: SubscriptionCloseReason,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventSubscriptionItem {
    Lifecycle(SubscriptionLifecycle),
    Event { session_id: u64, event: AppEvent },
}

pub struct EventSubscription {
    session_id: u64,
    opened_pending: bool,
    receiver: broadcast::Receiver<EventSubscriptionItem>,
}

impl EventSubscription {
    pub(crate) fn new(
        session_id: u64,
        receiver: broadcast::Receiver<EventSubscriptionItem>,
    ) -> Self {
        Self {
            session_id,
            opened_pending: true,
            receiver,
        }
    }

    pub fn session_id(&self) -> u64 {
        self.session_id
    }

    pub async fn recv(&mut self) -> Result<EventSubscriptionItem, broadcast::error::RecvError> {
        if self.opened_pending {
            self.opened_pending = false;
            return Ok(EventSubscriptionItem::Lifecycle(
                SubscriptionLifecycle::Opened {
                    session_id: self.session_id,
                },
            ));
        }
        self.receiver.recv().await
    }

    pub fn try_recv(&mut self) -> Result<EventSubscriptionItem, broadcast::error::TryRecvError> {
        if self.opened_pending {
            self.opened_pending = false;
            return Ok(EventSubscriptionItem::Lifecycle(
                SubscriptionLifecycle::Opened {
                    session_id: self.session_id,
                },
            ));
        }
        self.receiver.try_recv()
    }
}
