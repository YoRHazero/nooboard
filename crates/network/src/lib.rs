//! Authenticated TLS transport and bounded wire messages. No clipboard or storage access.
pub mod discovery;
mod identity;
pub mod local;
pub mod pairing;
mod protocol;
mod transport;
pub use identity::{Identity, fingerprint, new_session_id, noob_id};
pub use protocol::{
    ContentKind, ContentResult, FileEntry, MAX_CHUNK_BYTES, Manifest, TransferError,
};
pub use protocol::{MAX_TEXT_BYTES, Message, MessageId, PROTOCOL_VERSION, valid_device_name};
pub use transport::{Connection, ConnectionReceiver, ConnectionSender, TlsConfig};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("network I/O failed")]
    Io(#[from] std::io::Error),
    #[error("TLS configuration failed")]
    Tls(#[from] rustls::Error),
    #[error("invalid identity")]
    Identity,
    #[error("invalid protocol message")]
    Protocol,
    #[error("network operation timed out")]
    Timeout,
    #[error("connection closed")]
    Closed,
}
pub type Result<T> = std::result::Result<T, Error>;
