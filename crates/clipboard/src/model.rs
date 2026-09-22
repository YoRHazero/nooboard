use crate::{Error, ImageData};
use std::path::PathBuf;

/// Data that can be published to the native clipboard. File paths are references,
/// not file contents; copying or transmitting the files belongs to the caller.
#[derive(Clone, PartialEq, Eq)]
pub enum Payload {
    Text(String),
    Image(ImageData),
    Files(Vec<PathBuf>),
}
impl std::fmt::Debug for Payload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Text(text) => f.debug_struct("Text").field("bytes", &text.len()).finish(),
            Self::Image(image) => f.debug_tuple("Image").field(image).finish(),
            Self::Files(paths) => f
                .debug_struct("Files")
                .field("count", &paths.len())
                .finish(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkipReason {
    Sensitive,
    Unsupported,
    TooLarge,
    InvalidData,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReadState {
    Ready(Payload),
    Empty,
    Skipped(SkipReason),
}
impl ReadState {
    pub fn into_payload(self) -> Option<Payload> {
        match self {
            Self::Ready(payload) => Some(payload),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    /// Content not attributed to a write through this service instance,
    /// including existing clipboard content observed on startup.
    External,
    /// A confirmed write through this service instance, still at its native revision.
    Application,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    /// Monotonically increasing observation number within ONE service instance.
    /// Never compare revisions produced by different services or across restarts.
    pub revision: u64,
    pub content: ReadState,
    pub origin: Origin,
}

/// Operational health is separate from the last successfully observed snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ServiceStatus {
    Starting,
    Ready,
    Unavailable(Error),
    Stopped { error: Option<Error> },
}
