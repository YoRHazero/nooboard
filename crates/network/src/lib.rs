//! Authenticated TLS transport and bounded wire messages. No clipboard or storage access.
mod identity;
mod protocol;
mod transport;
pub use identity::{Identity, fingerprint};
pub use protocol::{MAX_TEXT_BYTES, Message, PROTOCOL_VERSION};
pub use transport::{Connection, TlsConfig};

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
