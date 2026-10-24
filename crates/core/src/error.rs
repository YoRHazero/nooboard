pub type Result<T> = std::result::Result<T, Error>;
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("clipboard operation failed: {0}")]
    Clipboard(#[from] nooboard_clipboard::Error),
    #[error("network operation failed: {0}")]
    Network(#[from] nooboard_network::Error),
    #[error("storage operation failed: {0}")]
    Storage(#[from] nooboard_storage::Error),
    #[error("backend stopped")]
    Stopped,
    #[error("peer is offline")]
    Offline,
    #[error("no send targets selected")]
    NoTargets,
    #[error("capacity exhausted")]
    Busy,
    #[error("synchronization is paused")]
    Paused,
    #[error("unsupported clipboard content")]
    Ineligible,
    #[error("invalid configuration")]
    Configuration,
    #[error("peer identity differs")]
    AlreadyPaired,
    #[error("certificate fingerprint does not match")]
    Fingerprint,
    #[error("record or operation not found")]
    NotFound,
    #[error("internal task failed")]
    Internal,
}
