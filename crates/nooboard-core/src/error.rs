use thiserror::Error;

pub type CoreResult<T> = Result<T, CoreError>;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("config error: {0}")]
    Config(#[from] nooboard_config::ConfigError),
    #[error("storage error: {0}")]
    Storage(#[from] nooboard_storage::StorageError),
    #[error("network error: {0}")]
    Network(#[from] nooboard_network::NetworkError),
    #[error("clipboard error: {0}")]
    Clipboard(String),
    #[error("channel closed: {0}")]
    ChannelClosed(String),
    #[error("invalid state: {0}")]
    InvalidState(String),
    #[error("clipboard event `{event_id}` was not found")]
    EventNotFound { event_id: String },
    #[error("session `{session_id}` was not found")]
    SessionNotFound { session_id: String },
    #[error("transfer `{ticket}` was not found")]
    TransferNotFound { ticket: String },
    #[error("transfer `{ticket}` cannot be cancelled")]
    TransferNotCancelable { ticket: String },
    #[error("clipboard text exceeds max_text_bytes: actual={actual_bytes}, max={max_bytes}")]
    TextTooLarge {
        actual_bytes: usize,
        max_bytes: usize,
    },
}
