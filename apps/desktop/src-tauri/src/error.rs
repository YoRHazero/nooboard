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
    #[error(transparent)]
    Core(#[from] nooboard_core::Error),
}
pub type Result<T> = std::result::Result<T, Error>;
pub type Future<'a, T> =
    std::pin::Pin<Box<dyn std::future::Future<Output = Result<T>> + Send + 'a>>;
