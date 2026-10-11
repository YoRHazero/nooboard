use crate::{Error, RemotePath};
use serde::Serialize;
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize)]
pub enum TransferSpec {
    Upload {
        local: PathBuf,
        remote_directory: RemotePath,
    },
    Download {
        remote: RemotePath,
        local_directory: PathBuf,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum TransferState {
    Queued,
    WaitingForConnection,
    Running,
    Finalizing,
    Succeeded,
    Failed,
    Cancelled,
    Unconfirmed,
}
impl TransferState {
    pub fn finished(self) -> bool {
        matches!(
            self,
            Self::Succeeded | Self::Failed | Self::Cancelled | Self::Unconfirmed
        )
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct Transfer {
    pub id: String,
    pub session_id: String,
    pub device_id: String,
    pub spec: TransferSpec,
    pub state: TransferState,
    pub transferred: u64,
    pub total: Option<u64>,
    pub target: Option<String>,
    /// Retained if cleanup could not be confirmed. Never delete unrelated files.
    pub temporary_path: Option<String>,
    pub error: Option<Error>,
}
