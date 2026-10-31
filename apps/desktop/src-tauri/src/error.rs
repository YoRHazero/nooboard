#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("backend is not connected")]
    BackendUnavailable,
    #[error("application is closing")]
    Closing,
    #[error("desktop preference operation failed")]
    Preferences,
    #[error("tray is unsupported")]
    TrayUnsupported,
    #[error("subscription was superseded or unavailable")]
    Subscription,
    #[error("application directory unavailable")]
    AppDirectory,
    #[error("diagnostics are disabled")]
    DiagnosticsDisabled,
    #[error("no verified update is available")]
    UpdateUnavailable,
    #[error("application update could not be installed")]
    UpdateInstall,
    #[error(transparent)]
    Core(#[from] nooboard_core::Error),
}
pub type Result<T> = std::result::Result<T, Error>;
pub type Future<'a, T> =
    std::pin::Pin<Box<dyn std::future::Future<Output = Result<T>> + Send + 'a>>;

impl From<nooboard_core::terminal::TerminalError> for Error {
    fn from(error: nooboard_core::terminal::TerminalError) -> Self {
        Self::Core(nooboard_core::Error::Terminal(error))
    }
}
