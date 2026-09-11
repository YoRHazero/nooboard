use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Manual,
    Automatic,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Settings {
    pub mode: Mode,
    pub receive: bool,
    pub paused: bool,
    pub history: bool,
    pub max_history_entries: u32,
    pub history_days: u32,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            mode: Mode::Manual,
            receive: true,
            paused: false,
            history: true,
            max_history_entries: 1000,
            history_days: 30,
        }
    }
}
impl Settings {
    pub(crate) fn validate(&self) -> crate::Result<()> {
        if self.max_history_entries == 0
            || self.max_history_entries > 100_000
            || self.history_days == 0
            || self.history_days > 3650
        {
            Err(crate::Error::Configuration)
        } else {
            Ok(())
        }
    }
    pub(crate) fn accepting(&self) -> bool {
        self.receive && !self.paused
    }
}
pub struct Options {
    /// Use a private, per-user directory. History is stored as plaintext SQLite.
    pub database: PathBuf,
    /// Stable OS credential entry name. Different profiles have different identities.
    pub profile: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Endpoint {
    Listen(String),
    Connect(String),
}
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Peer {
    pub certificate: Vec<u8>,
    pub endpoint: Endpoint,
}
pub struct PairRequest {
    pub certificate: Vec<u8>,
    /// SHA-256 shown on the other device, checked independently before confirmation.
    pub confirmed_fingerprint: String,
    pub endpoint: Endpoint,
}
#[derive(Clone, Debug)]
pub struct Status {
    pub fingerprint: String,
    pub peer_fingerprint: Option<String>,
    pub online: bool,
    pub peer_accepting: bool,
    pub settings: Settings,
}
#[derive(Clone, Debug)]
pub enum Event {
    Status(Status),
    Sent { sequence: u64 },
    Applied { sequence: u64 },
    Rejected { sequence: u64 },
    Received { bytes: usize },
    HistoryChanged,
    Fault(String),
}
#[derive(Clone, PartialEq, Eq)]
pub struct HistoryEntry {
    pub id: i64,
    pub text: String,
    pub source: String,
    pub copied_at_ms: i64,
}
impl std::fmt::Debug for HistoryEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HistoryEntry")
            .field("id", &self.id)
            .field("bytes", &self.text.len())
            .field("source", &self.source)
            .field("copied_at_ms", &self.copied_at_ms)
            .finish()
    }
}
impl From<nooboard_storage::HistoryEntry> for HistoryEntry {
    fn from(entry: nooboard_storage::HistoryEntry) -> Self {
        Self {
            id: entry.id,
            text: entry.text,
            source: entry.source,
            copied_at_ms: entry.copied_at_ms,
        }
    }
}
