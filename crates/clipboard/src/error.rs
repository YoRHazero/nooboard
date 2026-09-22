#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("clipboard is temporarily unavailable")]
    Unavailable,
    #[error("clipboard changed during the operation")]
    Changed,
    #[error("clipboard operation timed out")]
    Timeout,
    #[error("clipboard service stopped")]
    Stopped,
    #[error("invalid clipboard payload or options")]
    InvalidInput,
    #[error("invalid native clipboard data")]
    InvalidData,
    #[error("clipboard platform is unsupported")]
    UnsupportedPlatform,
    #[error("clipboard session is unsupported: {0}")]
    UnsupportedSession(&'static str),
    #[error("clipboard {operation} failed: {detail}")]
    Backend {
        operation: &'static str,
        detail: std::sync::Arc<str>,
    },
}
impl Error {
    pub(crate) fn backend(operation: &'static str, error: impl std::fmt::Display) -> Self {
        Self::Backend {
            operation,
            detail: error.to_string().into(),
        }
    }
    pub(crate) fn retryable(&self) -> bool {
        matches!(self, Self::Unavailable | Self::Changed | Self::Timeout)
    }
}
pub type Result<T> = std::result::Result<T, Error>;
