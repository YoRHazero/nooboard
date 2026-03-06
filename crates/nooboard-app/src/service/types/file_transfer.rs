use std::path::PathBuf;

use super::{NoobId, Targets};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendFileRequest {
    pub path: PathBuf,
    pub targets: Targets,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDecisionRequest {
    pub peer_noob_id: NoobId,
    pub transfer_id: u32,
    pub accept: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransferDirection {
    Incoming,
    Outgoing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransferState {
    Started {
        file_name: String,
        total_bytes: u64,
    },
    Progress {
        done_bytes: u64,
        total_bytes: u64,
        bps: Option<u64>,
        eta_ms: Option<u64>,
    },
    Finished {
        path: Option<PathBuf>,
    },
    Failed {
        reason: String,
    },
    Cancelled {
        reason: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferUpdate {
    pub transfer_id: u32,
    pub peer_noob_id: NoobId,
    pub direction: TransferDirection,
    pub state: TransferState,
}
