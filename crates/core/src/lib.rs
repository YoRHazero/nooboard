//! UI-independent application backend. Only this crate coordinates the other libraries.
mod app;
mod bootstrap;
mod history;
mod link;
mod model;
mod ports;
mod runtime;
mod sync;
pub use app::App;
pub use model::{Endpoint, Event, HistoryEntry, Mode, Options, PairRequest, Settings, Status};
pub use nooboard_network::fingerprint as certificate_fingerprint;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("clipboard operation failed: {0}")]
    Clipboard(#[from] nooboard_clipboard::Error),
    #[error("network operation failed: {0}")]
    Network(#[from] nooboard_network::Error),
    #[error("storage operation failed: {0}")]
    Storage(#[from] nooboard_storage::Error),
    #[error("native identity storage failed")]
    Secret(#[from] nooboard_storage::secrets::SecretError),
    #[error("backend stopped")]
    Stopped,
    #[error("peer is offline or not ready to receive")]
    Offline,
    #[error("synchronization is paused")]
    Paused,
    #[error("clipboard does not contain eligible text")]
    Ineligible,
    #[error("invalid configuration")]
    Configuration,
    #[error("peer identity differs; unpair before replacing it")]
    AlreadyPaired,
    #[error("certificate fingerprint does not match confirmation")]
    Fingerprint,
    #[error("history record does not exist")]
    NotFound,
}
pub type Result<T> = std::result::Result<T, Error>;
#[cfg(test)]
mod tests;
