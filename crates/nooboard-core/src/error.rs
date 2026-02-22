use thiserror::Error;

#[derive(Debug, Error)]
pub enum NooboardError {
    #[error("platform error: {0}")]
    Platform(String),
    #[error("storage error: {0}")]
    Storage(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("channel closed: {0}")]
    Channel(String),
    #[error("unsupported platform")]
    UnsupportedPlatform,
}

impl NooboardError {
    pub fn platform(message: impl Into<String>) -> Self {
        Self::Platform(message.into())
    }

    pub fn storage(message: impl Into<String>) -> Self {
        Self::Storage(message.into())
    }

    pub fn channel(message: impl Into<String>) -> Self {
        Self::Channel(message.into())
    }
}
