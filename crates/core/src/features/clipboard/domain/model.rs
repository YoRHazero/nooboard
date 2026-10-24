use crate::ContentKind;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum DeliveryState {
    Queued,
    Sending,
    AwaitingReceipt,
    Applied,
    Rejected,
    Unconfirmed,
    Offline,
    Cancelled,
    Superseded,
    QueueFull,
}
impl DeliveryState {
    pub fn pending(self) -> bool {
        matches!(self, Self::Queued | Self::Sending | Self::AwaitingReceipt)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Delivery {
    pub noob_id: String,
    pub device_name: String,
    pub state: DeliveryState,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Transfer {
    pub id: crate::MessageId,
    pub automatic: bool,
    pub bytes: usize,
    pub targets: Vec<Delivery>,
}
impl Transfer {
    pub fn pending(&self) -> bool {
        self.targets.iter().any(|d| d.state.pending())
    }
}
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
pub enum ContentStage {
    Preparing,
    Queued,
    Waiting,
    Sending,
    Receiving,
    Verifying,
    Saving,
    Applying,
    Cancelling,
    Completed,
    Saved,
    Failed,
    Cancelled,
    Unconfirmed,
}
impl ContentStage {
    pub fn pending(self) -> bool {
        matches!(
            self,
            Self::Preparing
                | Self::Queued
                | Self::Waiting
                | Self::Sending
                | Self::Receiving
                | Self::Verifying
                | Self::Saving
                | Self::Applying
                | Self::Cancelling
        )
    }
    pub fn cancellable(self) -> bool {
        self.pending() && !matches!(self, Self::Saving | Self::Applying | Self::Cancelling)
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct ContentTransfer {
    pub key: String,
    pub id: crate::MessageId,
    pub peer: String,
    pub device_name: String,
    pub incoming: bool,
    pub kind: ContentKind,
    pub names: Vec<String>,
    pub total_bytes: u64,
    pub completed_bytes: u64,
    pub prepared_bytes: u64,
    pub stage: ContentStage,
    pub error: Option<TransferFailure>,
    pub saved_paths: Vec<PathBuf>,
    pub at_ms: i64,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
pub enum TransferFailure {
    Denied,
    Directory,
    Unsupported,
    TooLarge,
    SourceChanged,
    Integrity,
    Io,
    Clipboard,
    Offline,
    Timeout,
    Busy,
    Cancelled,
    Protocol,
}

use crate::contracts::network::{TransferStage, TransferStatus};
pub(crate) fn transfer_key(row: &TransferStatus) -> String {
    format!(
        "{}:{}:{}:{}",
        if row.incoming { "in" } else { "out" },
        row.id.session(),
        row.id.sequence(),
        row.peer
    )
}
pub(crate) fn content_stage(stage: TransferStage) -> ContentStage {
    match stage {
        TransferStage::Preparing => ContentStage::Preparing,
        TransferStage::Queued => ContentStage::Queued,
        TransferStage::Sending => ContentStage::Sending,
        TransferStage::Receiving => ContentStage::Receiving,
        TransferStage::WaitingForAcceptance => ContentStage::Waiting,
        TransferStage::WaitingForApplication => ContentStage::Applying,
        TransferStage::AwaitingReceipt => ContentStage::Waiting,
        TransferStage::Cancelling => ContentStage::Cancelling,
        TransferStage::Applied => ContentStage::Completed,
        TransferStage::Saved => ContentStage::Saved,
        TransferStage::Unconfirmed => ContentStage::Unconfirmed,
        TransferStage::Cancelled | TransferStage::Superseded => ContentStage::Cancelled,
        TransferStage::Failed | TransferStage::Rejected => ContentStage::Failed,
    }
}
