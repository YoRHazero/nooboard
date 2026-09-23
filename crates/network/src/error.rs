use std::{error::Error as StdError, fmt};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ErrorKind {
    InvalidInput,
    Busy,
    Stopped,
    NotFound,
    Identity,
    Credentials,
    Protocol,
    Unavailable,
    Timeout,
    Cancelled,
    TooLarge,
    SourceChanged,
    Integrity,
    Internal,
}
/// Driver diagnostics are available through `source`, never default formatting.
pub struct Error {
    kind: ErrorKind,
    source: Failure,
}
impl Error {
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }
}
impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Error").field("kind", &self.kind).finish()
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "network: {:?}", self.kind)
    }
}
impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        Some(&self.source)
    }
}
impl From<Failure> for Error {
    fn from(source: Failure) -> Self {
        Self {
            kind: source.kind(),
            source,
        }
    }
}
#[derive(Debug, thiserror::Error)]
pub(crate) enum Failure {
    #[error("network I/O failed")]
    Io(#[from] std::io::Error),
    #[error("TLS configuration failed")]
    Tls(#[from] rustls::Error),
    #[error("invalid identity")]
    Identity,
    #[error("credential operation failed")]
    Credentials,
    #[error("invalid protocol")]
    Protocol,
    #[error("operation timed out")]
    Timeout,
    #[error("service stopped")]
    Closed,
    #[error("peer is unavailable")]
    Disconnected,
    #[error("capacity exhausted")]
    Busy,
    #[error("operation not found")]
    NotFound,
    #[error("invalid input: {0}")]
    InvalidArgument(&'static str),
    #[error("operation cancelled")]
    Cancelled,
    #[error("content too large")]
    TooLarge,
    #[error("source changed")]
    SourceChanged,
    #[error("content integrity check failed")]
    Integrity,
    #[error("internal task failed")]
    Internal,
}
impl Failure {
    pub fn kind(&self) -> ErrorKind {
        match self {
            Self::Io(_) | Self::Disconnected => ErrorKind::Unavailable,
            Self::Tls(_) | Self::Identity => ErrorKind::Identity,
            Self::Credentials => ErrorKind::Credentials,
            Self::Protocol => ErrorKind::Protocol,
            Self::Timeout => ErrorKind::Timeout,
            Self::Closed => ErrorKind::Stopped,
            Self::Busy => ErrorKind::Busy,
            Self::NotFound => ErrorKind::NotFound,
            Self::InvalidArgument(_) => ErrorKind::InvalidInput,
            Self::Cancelled => ErrorKind::Cancelled,
            Self::TooLarge => ErrorKind::TooLarge,
            Self::SourceChanged => ErrorKind::SourceChanged,
            Self::Integrity => ErrorKind::Integrity,
            Self::Internal => ErrorKind::Internal,
        }
    }
}
pub(crate) type InternalResult<T> = std::result::Result<T, Failure>;
