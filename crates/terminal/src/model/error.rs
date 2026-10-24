use super::{FileOperationPath, RunId, TerminalError, TerminalErrorKind};
pub type Result<T> = std::result::Result<T, TerminalError>;
impl TerminalError {
    pub fn new(kind: TerminalErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            source_run_id: None,
            path: None,
            cause: None,
        }
    }
    pub(crate) fn invalid(message: impl Into<String>) -> Self {
        Self::new(TerminalErrorKind::InvalidRequest, message)
    }
    pub(crate) fn cancelled() -> Self {
        Self::new(TerminalErrorKind::Cancelled, "operation cancelled")
    }
    pub(crate) fn closed() -> Self {
        Self::new(
            TerminalErrorKind::ServiceShuttingDown,
            "terminal service is shutting down",
        )
    }
    pub(crate) fn at(mut self, path: FileOperationPath) -> Self {
        self.path = Some(path);
        self
    }
    pub(crate) fn with_source_run(mut self, run: RunId) -> Self {
        self.source_run_id = Some(run);
        self
    }
    pub(crate) fn dependency(run: RunId, cause: Self) -> Self {
        let mut error = Self::new(
            TerminalErrorKind::DependencyFailed,
            "prerequisite command ended",
        );
        error.source_run_id = Some(run);
        error.cause = Some(Box::new(cause));
        error
    }
}
impl std::fmt::Display for TerminalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.message.fmt(f)
    }
}
impl std::error::Error for TerminalError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.cause.as_deref().map(|e| e as _)
    }
}
impl From<std::io::Error> for TerminalError {
    fn from(error: std::io::Error) -> Self {
        use std::io::ErrorKind;
        let kind = match error.kind() {
            ErrorKind::PermissionDenied => TerminalErrorKind::PermissionDenied,
            ErrorKind::NotFound => TerminalErrorKind::PathNotFound,
            ErrorKind::AlreadyExists => TerminalErrorKind::Conflict,
            _ => TerminalErrorKind::Other,
        };
        Self::new(kind, error.to_string())
    }
}

impl std::fmt::Display for super::TerminalShutdownError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "terminal shutdown failed with {} pending runs",
            self.pending_runs.len()
        )?;
        if let Some(error) = self.errors.first() {
            write!(f, ": {error}")?;
        }
        Ok(())
    }
}
impl std::error::Error for super::TerminalShutdownError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.errors.first().map(|e| e as _)
    }
}
