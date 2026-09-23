use crate::error::{ErrorKind, Failure, InternalResult as Result};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, path::PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(deny_unknown_fields)]
pub struct TransferId {
    pub(crate) session: String,
    pub(crate) sequence: u64,
}
impl TransferId {
    pub fn session(&self) -> &str {
        &self.session
    }
    pub fn sequence(&self) -> u64 {
        self.sequence
    }
    pub(crate) fn valid(&self) -> bool {
        self.sequence != 0
            && self.session.len() == 32
            && self
                .session
                .bytes()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IncomingId(pub(crate) u64);
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum QueuePolicy {
    #[default]
    Append,
    /// Replace only the adjacent, unstarted text at the queue tail with this key.
    ReplaceTail(String),
}
pub enum OutgoingContent {
    Text(String),
    Image(Vec<u8>),
    Files(Vec<PathBuf>),
}
impl std::fmt::Debug for OutgoingContent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Text(v) => f.debug_tuple("TextBytes").field(&v.len()).finish(),
            Self::Image(v) => f.debug_tuple("ImageBytes").field(&v.len()).finish(),
            Self::Files(v) => f.debug_tuple("FileCount").field(&v.len()).finish(),
        }
    }
}
#[derive(Debug)]
pub struct SendRequest {
    pub targets: Vec<String>,
    pub content: OutgoingContent,
    pub queue: QueuePolicy,
}
impl SendRequest {
    pub fn text(targets: Vec<String>, text: impl Into<String>) -> Self {
        Self {
            targets,
            content: OutgoingContent::Text(text.into()),
            queue: QueuePolicy::Append,
        }
    }
    pub(crate) fn validate(&self) -> Result<()> {
        let mut peers = HashSet::new();
        if self.targets.is_empty()
            || self.targets.len() > 64
            || self.targets.iter().any(|id| {
                id.len() != 64
                    || !id
                        .bytes()
                        .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
                    || !peers.insert(id)
            })
        {
            return Err(Failure::InvalidArgument("targets"));
        }
        if let QueuePolicy::ReplaceTail(key) = &self.queue
            && (key.is_empty()
                || key.len() > 256
                || key.contains('\0')
                || !matches!(self.content, OutgoingContent::Text(_)))
        {
            return Err(Failure::InvalidArgument("queue policy"));
        }
        match &self.content {
            OutgoingContent::Text(text) if text.len() > MAX_TEXT_BYTES || text.contains('\0') => {
                Err(Failure::InvalidArgument("text"))
            }
            OutgoingContent::Image(bytes)
                if bytes.len() > 64 * 1024 * 1024 || !bytes.starts_with(b"\x89PNG\r\n\x1a\n") =>
            {
                Err(Failure::InvalidArgument("PNG image"))
            }
            OutgoingContent::Files(paths)
                if paths.is_empty()
                    || paths.len() > 256
                    || paths.iter().any(|p| p.as_os_str().is_empty()) =>
            {
                Err(Failure::InvalidArgument("files"))
            }
            _ => Ok(()),
        }
    }
}
#[derive(Debug)]
pub enum ReceiveDecision {
    Reject,
    Accept { directory: Option<PathBuf> },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationOutcome {
    Applied,
    Saved,
    Rejected,
    Failed,
}
pub enum ReceivedContent {
    Text(String),
    Image(Vec<u8>),
    Files(Vec<PathBuf>),
}
impl std::fmt::Debug for ReceivedContent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Text(v) => f.debug_tuple("TextBytes").field(&v.len()).finish(),
            Self::Image(v) => f.debug_tuple("ImageBytes").field(&v.len()).finish(),
            Self::Files(v) => f.debug_tuple("FileCount").field(&v.len()).finish(),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransferStage {
    Preparing,
    Queued,
    Sending,
    WaitingForAcceptance,
    Receiving,
    WaitingForApplication,
    AwaitingReceipt,
    Cancelling,
    Applied,
    Saved,
    Rejected,
    Cancelled,
    Superseded,
    Unconfirmed,
    Failed,
}
impl TransferStage {
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Applied
                | Self::Saved
                | Self::Rejected
                | Self::Cancelled
                | Self::Superseded
                | Self::Unconfirmed
                | Self::Failed
        )
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransferStatus {
    pub id: TransferId,
    pub peer: String,
    pub incoming: bool,
    pub stage: TransferStage,
    pub completed_bytes: u64,
    pub total_bytes: u64,
    pub error: Option<ErrorKind>,
    pub saved_paths: Vec<PathBuf>,
}

pub const MAX_TEXT_BYTES: usize = 1024 * 1024;
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ContentKind {
    Image,
    Files,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FileEntry {
    pub name: String,
    pub bytes: u64,
    pub sha256: [u8; 32],
}
