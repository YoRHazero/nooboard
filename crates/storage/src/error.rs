use std::{error::Error as StdError, fmt};

/// Stable categories. Driver errors remain available through `Error::source`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ErrorKind {
    InvalidInput,
    Busy,
    Conflict,
    Unavailable,
    PermissionDenied,
    Corrupt,
    SchemaTooNew,
    Stopped,
    Internal,
}

pub struct Error {
    kind: ErrorKind,
    operation: &'static str,
    source: Option<Box<dyn StdError + Send + Sync>>,
}
impl Error {
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }
    pub fn operation(&self) -> &'static str {
        self.operation
    }
    pub(crate) fn new(kind: ErrorKind, operation: &'static str) -> Self {
        Self {
            kind,
            operation,
            source: None,
        }
    }
    pub(crate) fn caused_by(
        kind: ErrorKind,
        operation: &'static str,
        source: impl StdError + Send + Sync + 'static,
    ) -> Self {
        Self {
            kind,
            operation,
            source: Some(Box::new(source)),
        }
    }
    pub(crate) fn invalid(argument: &'static str) -> Self {
        Self::new(ErrorKind::InvalidInput, argument)
    }
    pub(crate) fn stopped() -> Self {
        Self::new(ErrorKind::Stopped, "storage service")
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {:?}", self.operation, self.kind)
    }
}
// Driver diagnostics may contain data. Default formatting only exposes categories.
impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Error")
            .field("kind", &self.kind)
            .field("operation", &self.operation)
            .finish()
    }
}
impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.source
            .as_deref()
            .map(|e| e as &(dyn StdError + 'static))
    }
}
pub type Result<T> = std::result::Result<T, Error>;
