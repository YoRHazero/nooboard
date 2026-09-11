#[derive(Clone, PartialEq, Eq)]
pub enum Content {
    Text(String),
    Empty,
    Unsupported,
    Sensitive,
    TooLarge,
}
impl std::fmt::Debug for Content {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Text(text) => f.debug_struct("Text").field("bytes", &text.len()).finish(),
            Self::Empty => f.write_str("Empty"),
            Self::Unsupported => f.write_str("Unsupported"),
            Self::Sensitive => f.write_str("Sensitive"),
            Self::TooLarge => f.write_str("TooLarge"),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    External,
    Application,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub revision: u64,
    pub content: Content,
    pub origin: Origin,
}
#[derive(Clone, Debug, thiserror::Error)]
pub enum Error {
    #[error("clipboard is temporarily unavailable")]
    Unavailable,
    #[error("clipboard changed during the operation")]
    Changed,
    #[error("clipboard worker stopped")]
    Stopped,
    #[error("invalid clipboard text or options")]
    InvalidInput,
    #[error("clipboard platform is unsupported")]
    UnsupportedPlatform,
    #[error("native clipboard operation failed")]
    Native,
}
pub type Result<T> = std::result::Result<T, Error>;
