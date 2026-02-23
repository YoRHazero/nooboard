use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("storage error: {0}")]
    Storage(String),
    #[error("platform error: {0}")]
    Platform(String),
    #[error("sync error: {0}")]
    Sync(String),
    #[error("sync is already running")]
    SyncAlreadyRunning,
    #[error("sync is not running")]
    SyncNotRunning,
    #[error("runtime error: {0}")]
    Runtime(String),
    #[error("unsupported platform")]
    UnsupportedPlatform,
}

impl From<nooboard_storage::StorageError> for AppError {
    fn from(value: nooboard_storage::StorageError) -> Self {
        Self::Storage(value.to_string())
    }
}

impl From<nooboard_core::NooboardError> for AppError {
    fn from(value: nooboard_core::NooboardError) -> Self {
        match value {
            nooboard_core::NooboardError::UnsupportedPlatform => Self::UnsupportedPlatform,
            _ => Self::Platform(value.to_string()),
        }
    }
}

impl From<nooboard_sync::SyncError> for AppError {
    fn from(value: nooboard_sync::SyncError) -> Self {
        Self::Sync(value.to_string())
    }
}
