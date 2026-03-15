use thiserror::Error;

use crate::{DirectRequestId, DirectSeedId, SessionId, TransferTicket};

pub type NetworkResult<T> = Result<T, NetworkError>;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum NetworkError {
    #[error("invalid config: {0}")]
    InvalidConfig(String),
    #[error("network runtime is not running")]
    NotRunning,
    #[error("direct seed not found: {0}")]
    DirectSeedNotFound(DirectSeedId),
    #[error("direct request not found: {0}")]
    DirectRequestNotFound(DirectRequestId),
    #[error("session not found: {0}")]
    SessionNotFound(SessionId),
    #[error("transfer not found: {0}:{1}")]
    TransferNotFound(SessionId, u32),
    #[error("transfer cannot be changed in its current state: {0}:{1}")]
    TransferNotCancelable(SessionId, u32),
    #[error("channel closed")]
    ChannelClosed,
    #[error("internal error: {0}")]
    Internal(String),
}

impl NetworkError {
    pub(crate) fn transfer_not_found(ticket: TransferTicket) -> Self {
        Self::TransferNotFound(ticket.session_id, ticket.raw_id)
    }

    pub(crate) fn transfer_not_cancelable(ticket: TransferTicket) -> Self {
        Self::TransferNotCancelable(ticket.session_id, ticket.raw_id)
    }
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum NetworkEventRecvError {
    #[error("network event channel closed")]
    Closed,
}

#[derive(Debug, Error)]
pub(crate) enum ProtocolError {
    #[error("packet serialization failed: {0}")]
    Serialize(#[from] postcard::Error),
    #[error("packet deserialization failed: {0}")]
    Deserialize(postcard::Error),
    #[error("unauthenticated connection only accepts Packet::Handshake")]
    HandshakeRequired,
}

#[derive(Debug, Error)]
pub(crate) enum TransportError {
    #[error("TLS error: {0}")]
    Rustls(#[from] rustls::Error),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("protocol error: {0}")]
    Protocol(#[from] ProtocolError),
    #[error("invalid DNS name for TLS: {0}")]
    InvalidServerName(String),
}

#[derive(Debug, Error)]
pub(crate) enum DiscoveryError {
    #[error("mDNS error: {0}")]
    Mdns(String),
}

#[derive(Debug, Error)]
pub(crate) enum FileReceiveError {
    #[error("path `{0}` is invalid")]
    InvalidFileName(String),
    #[error("file size {size} exceeds max {max}")]
    FileTooLarge { size: u64, max: u64 },
    #[error("too many active downloads")]
    TooManyActiveDownloads,
    #[error("transfer {0} already exists")]
    DuplicateTransfer(u32),
    #[error("transfer {0} does not exist")]
    UnknownTransfer(u32),
    #[error("transfer {0} is waiting for decision")]
    DecisionRequired(u32),
    #[error("transfer {0} decision already made")]
    DecisionAlreadyMade(u32),
    #[error("transfer {transfer_id} chunk out of order: expected {expected}, got {got}")]
    OutOfOrderChunk {
        transfer_id: u32,
        expected: u32,
        got: u32,
    },
    #[error("received bytes mismatch: expected {expected}, got {actual}")]
    SizeMismatch { expected: u64, actual: u64 },
    #[error("received chunk count mismatch: expected {expected}, got {actual}")]
    ChunkCountMismatch { expected: u32, actual: u32 },
    #[error("checksum mismatch: expected {expected}, got {actual}")]
    ChecksumMismatch { expected: String, actual: String },
    #[error("unsafe path escaped download dir")]
    UnsafePath,
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Error)]
pub(crate) enum ConnectionError {
    #[error("transport error: {0}")]
    Transport(#[from] TransportError),
    #[error("file receive error: {0}")]
    FileReceive(#[from] FileReceiveError),
    #[error("pong timeout")]
    PongTimeout,
    #[error("connection state error: {0}")]
    State(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}
