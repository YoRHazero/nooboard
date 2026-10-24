pub type Result<T> = std::result::Result<T, Error>;

/// Errors never include key material, plaintext, or credential-store diagnostics.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("invalid identity options or request")]
    InvalidInput,
    #[error("invalid identity material")]
    InvalidIdentity,
    #[error("identity credential store is unavailable")]
    Credentials,
    #[error("identity credential storage is not enabled")]
    UnsupportedStorage,
    #[error("identity request queue or credential is busy")]
    Busy,
    #[error("identity service stopped")]
    Stopped,
    #[error("identity operation timed out")]
    Timeout,
    #[error("unsupported identity signature scheme")]
    UnsupportedSignature,
    #[error("identity signature failed")]
    Signing,
    #[error("identity encryption failed")]
    Encryption,
    #[error("sealed secret authentication failed")]
    Authentication,
    #[error("unsupported sealed secret version")]
    UnsupportedVersion,
    #[error("identity worker failed")]
    Internal,
}
