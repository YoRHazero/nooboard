use std::net::SocketAddr;

use tokio::sync::broadcast;

use super::{ClipboardRecordSource, NoobId, TransferId, TransferOutcome};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppEvent {
    ClipboardCommitted {
        event_id: super::EventId,
        source: ClipboardRecordSource,
    },
    IncomingTransferOffered {
        transfer_id: TransferId,
    },
    TransferUpdated {
        transfer_id: TransferId,
    },
    TransferCompleted {
        transfer_id: TransferId,
        outcome: TransferOutcome,
    },
    PeerConnectionError {
        peer_noob_id: Option<NoobId>,
        addr: Option<SocketAddr>,
        error: String,
    },
}

pub type EventRecvError = broadcast::error::RecvError;

pub struct EventSubscription {
    receiver: broadcast::Receiver<AppEvent>,
}

impl EventSubscription {
    pub(crate) fn new(receiver: broadcast::Receiver<AppEvent>) -> Self {
        Self { receiver }
    }

    pub async fn recv(&mut self) -> Result<AppEvent, EventRecvError> {
        self.receiver.recv().await
    }
}
