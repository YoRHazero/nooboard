use std::path::PathBuf;

use super::{NoobId, TransferId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendFileItem {
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendFilesRequest {
    pub targets: Vec<NoobId>,
    pub files: Vec<SendFileItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IncomingTransferDisposition {
    Accept,
    Reject,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncomingTransferDecision {
    pub transfer_id: TransferId,
    pub decision: IncomingTransferDisposition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferDirection {
    Upload,
    Download,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferState {
    Queued,
    Starting,
    InProgress,
    Cancelling,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferOutcome {
    Succeeded,
    Rejected,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncomingTransfer {
    pub transfer_id: TransferId,
    pub peer_noob_id: NoobId,
    pub file_name: String,
    pub file_size: u64,
    pub total_chunks: u32,
    pub offered_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transfer {
    pub transfer_id: TransferId,
    pub direction: TransferDirection,
    pub peer_noob_id: NoobId,
    pub file_name: String,
    pub file_size: u64,
    pub transferred_bytes: u64,
    pub state: TransferState,
    pub started_at_ms: i64,
    pub updated_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletedTransfer {
    pub transfer_id: TransferId,
    pub direction: TransferDirection,
    pub peer_noob_id: NoobId,
    pub file_name: String,
    pub file_size: u64,
    pub outcome: TransferOutcome,
    pub started_at_ms: Option<i64>,
    pub finished_at_ms: i64,
    pub saved_path: Option<PathBuf>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TransfersState {
    pub incoming_pending: Vec<IncomingTransfer>,
    pub active: Vec<Transfer>,
    pub recent_completed: Vec<CompletedTransfer>,
}
