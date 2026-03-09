use std::path::PathBuf;

use nooboard_platform::NooboardError;
use nooboard_storage::StorageError;
use nooboard_sync::SyncError;
use thiserror::Error;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("storage error: {0}")]
    Storage(#[from] StorageError),
    #[error("sync error: {0}")]
    Sync(#[from] SyncError),
    #[error("platform error: {0}")]
    Platform(#[from] NooboardError),
    #[error("failed to parse app config `{path}`: {source}")]
    ConfigParse {
        path: PathBuf,
        source: toml::de::Error,
    },
    #[error("failed to serialize app config: {0}")]
    ConfigSerialize(#[from] toml::ser::Error),
    #[error("invalid app config: {0}")]
    InvalidConfig(String),
    #[error("sync engine is not running")]
    EngineNotRunning,
    #[error("sync engine is already running")]
    EngineAlreadyRunning,
    #[error("sync network is disabled")]
    SyncDisabled,
    #[error("sync channel closed: {0}")]
    ChannelClosed(String),
    #[error("history event `{event_id}` was not found")]
    EventNotFound { event_id: String },
    #[error("invalid event id `{event_id}`: expected UUID string")]
    InvalidEventId { event_id: String },
    #[error("clipboard text exceeds max_text_bytes: actual={actual_bytes}, max={max_bytes}")]
    TextTooLarge {
        actual_bytes: usize,
        max_bytes: usize,
    },
    #[error("peer `{peer_noob_id}` is not connected")]
    PeerNotConnected { peer_noob_id: String },
    #[error("transfer `{transfer_id}` was not found")]
    TransferNotFound { transfer_id: String },
    #[error("transfer `{transfer_id}` cannot be cancelled")]
    TransferNotCancelable { transfer_id: String },
    #[error("manual peer `{peer}` already exists")]
    ManualPeerExists { peer: String },
    #[error("manual peer `{peer}` does not exist")]
    ManualPeerNotFound { peer: String },
    #[error(
        "configuration restart failed and rollback also failed: restart={restart_error}; rollback={rollback_error}"
    )]
    ConfigRollbackFailed {
        restart_error: String,
        rollback_error: String,
    },
}
