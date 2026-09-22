//! UI-independent application backend. Only this crate coordinates the other libraries.
mod app;
mod bootstrap;
mod devices;
#[cfg(feature = "diagnostics")]
pub mod diagnostics;
mod history;
mod link;
mod local_network;
pub use local_network::{LocalAddress, LocalNetwork};
mod model;
mod onboarding;
pub use onboarding::{
    NearbyDevice, OnboardingSnapshot, PairingError, PairingFailure, PairingSession, PairingStage,
};
mod content_transfer;
mod ports;
mod preview;
mod runtime;
mod sync;
mod transfers;
pub use content_transfer::{ContentStage, ContentTransfer};
mod view;
pub use app::App;
pub(crate) use model::VerifiedPeer;
pub use model::{Event, HistoryEntry, Mode, Options, PeerSettings, PeerStatus, Settings, Status};
pub use nooboard_network::{MessageId, valid_device_name};
pub use transfers::{Delivery, DeliveryState, Transfer};
pub use view::{ActivityKind, ActivityRecord, AppSnapshot, ClipboardKind, CurrentClipboard, Fault};

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
    #[error("no send targets selected")]
    NoTargets,
    #[error("too many pending transfers or paired devices")]
    Busy,
    #[error("synchronization is paused")]
    Paused,
    #[error("clipboard does not contain supported content")]
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
