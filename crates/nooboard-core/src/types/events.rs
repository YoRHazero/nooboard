use tokio::sync::broadcast;

use super::{ClipboardRecordSource, EventId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceEvent {
    ClipboardCommitted {
        event_id: EventId,
        source: ClipboardRecordSource,
    },
    IncomingTransferOffered {
        ticket: nooboard_network::TransferTicket,
    },
    TransferUpdated {
        ticket: nooboard_network::TransferTicket,
    },
    TransferCompleted {
        ticket: nooboard_network::TransferTicket,
        outcome: nooboard_network::TransferOutcome,
    },
    NetworkConnectionFailed {
        failure: nooboard_network::ConnectionFailure,
    },
}

pub type EventRecvError = broadcast::error::RecvError;

pub struct EventSubscription {
    receiver: broadcast::Receiver<WorkspaceEvent>,
}

impl EventSubscription {
    pub(crate) fn new(receiver: broadcast::Receiver<WorkspaceEvent>) -> Self {
        Self { receiver }
    }

    pub async fn recv(&mut self) -> Result<WorkspaceEvent, EventRecvError> {
        self.receiver.recv().await
    }
}
