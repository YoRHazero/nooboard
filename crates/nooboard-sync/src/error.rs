use rustls::Error as RustlsError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("packet serialization failed: {0}")]
    Serialize(bincode::Error),
    #[error("packet deserialization failed: {0}")]
    Deserialize(bincode::Error),
    #[error("unauthenticated connection only accepts Packet::Handshake")]
    HandshakeRequired,
}

#[derive(Debug, Error)]
pub enum TransportError {
    #[error("TLS error: {0}")]
    Rustls(#[from] RustlsError),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("protocol error: {0}")]
    Protocol(#[from] ProtocolError),
    #[error("invalid DNS name for TLS: {0}")]
    InvalidServerName(String),
    #[error("TLS frame unexpectedly closed")]
    Closed,
}

#[derive(Debug, Error)]
pub enum DiscoveryError {
    #[error("discovery channel closed")]
    ChannelClosed,
    #[error("mDNS error: {0}")]
    Mdns(String),
}

#[derive(Debug, Error)]
pub enum FileReceiveError {
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
pub enum ConnectionError {
    #[error("transport error: {0}")]
    Transport(#[from] TransportError),
    #[error("file receive error: {0}")]
    FileReceive(#[from] FileReceiveError),
    #[error("pong timeout")]
    PongTimeout,
    #[error("connection state error: {0}")]
    State(String),
    #[error("I/O error: {0}")]
    Io(std::io::Error),
}

#[derive(Debug, Error)]
pub enum SyncError {
    #[error("invalid sync config: {0}")]
    InvalidConfig(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("discovery error: {0}")]
    Discovery(#[from] DiscoveryError),
    #[error("connection error: {0}")]
    Connection(#[from] ConnectionError),
    #[error("handshake transport error: {0}")]
    Handshake(#[from] TransportError),
    #[error("handshake failed: {0}")]
    HandshakeMessage(String),
    #[error("channel closed")]
    ChannelClosed,
}
