use thiserror::Error;

#[derive(Debug, Error)]
pub enum SyncError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("websocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("mDNS error: {0}")]
    Mdns(String),
    #[error("authentication failed")]
    AuthenticationFailed,
    #[error("protocol error: {0}")]
    Protocol(String),
    #[error("channel closed")]
    ChannelClosed,
    #[error("duplicate peer connection: {0}")]
    DuplicatePeerConnection(String),
    #[error("connection direction rejected for peer: {0}")]
    DirectionRejected(String),
    #[error("self connection rejected")]
    SelfConnection,
    #[error("storage error: {0}")]
    Storage(String),
    #[error("platform error: {0}")]
    Platform(String),
    #[error("address parse error: {0}")]
    AddressParse(#[from] std::net::AddrParseError),
}

impl From<nooboard_storage::StorageError> for SyncError {
    fn from(value: nooboard_storage::StorageError) -> Self {
        Self::Storage(value.to_string())
    }
}

impl From<nooboard_core::NooboardError> for SyncError {
    fn from(value: nooboard_core::NooboardError) -> Self {
        Self::Platform(value.to_string())
    }
}
