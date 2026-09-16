use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Manual,
    Automatic,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Settings {
    pub mode: Mode,
    pub receive: bool,
    pub paused: bool,
    pub history: bool,
    pub max_history_entries: u32,
    pub history_days: u32,
    pub device_name: String,
    pub listen_address: String,
    pub pairing_listen_address: String,
    pub discoverable: bool,
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
            device_name: "Nooboard".into(),
            listen_address: "0.0.0.0:24816".into(),
            pairing_listen_address: "0.0.0.0:24817".into(),
            discoverable: true,
        }
    }
}
impl Settings {
    pub(crate) fn validate(&self) -> crate::Result<()> {
        if self.max_history_entries == 0
            || self.max_history_entries > 100_000
            || self.history_days == 0
            || self.history_days > 3650
            || !nooboard_network::valid_device_name(&self.device_name)
            || self.listen_address.parse::<std::net::SocketAddr>().is_err()
            || self
                .pairing_listen_address
                .parse::<std::net::SocketAddr>()
                .is_err()
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
pub struct PeerSettings {
    /// Optional dial address; peers without one may still connect to our listener.
    pub address: Option<String>,
    pub auto_send: bool,
}
impl PeerSettings {
    pub(crate) fn validate(&self) -> crate::Result<()> {
        if self.address.as_ref().is_some_and(|a| {
            a.len() > 512
                || a.chars().any(|c| c.is_whitespace() || c.is_control())
                || a.rsplit_once(':').is_none_or(|(host, port)| {
                    host.is_empty() || port.parse::<u16>().map_or(true, |p| p == 0)
                })
        }) {
            Err(crate::Error::Configuration)
        } else {
            Ok(())
        }
    }
}
#[derive(Clone)]
pub struct VerifiedPeer {
    pub certificate: Vec<u8>,
    /// Full certificate SHA-256 confirmed out of band, independently of the address.
    pub confirmed_fingerprint: String,
    pub device_name: String,
    pub address: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PeerStatus {
    pub noob_id: String,
    pub device_name: String,
    pub fingerprint: String,
    pub settings: PeerSettings,
    pub online: bool,
    pub accepting: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Status {
    pub noob_id: String,
    pub fingerprint: String,
    /// Actual bound address, including the selected port when configured with port 0.
    pub listen_address: String,
    pub settings: Settings,
    pub peers: Vec<PeerStatus>,
    pub manual_targets: Vec<String>,
    pub transfers: Vec<crate::Transfer>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Event {
    Status(Status),
    Copied {
        revision: u64,
        bytes: usize,
    },
    /// Creation and subsequent per-target updates share the same batch message ID.
    Transfer(crate::Transfer),
    Received {
        source: String,
        device_name: String,
        id: nooboard_network::MessageId,
        bytes: usize,
    },
    HistoryChanged,
    Fault {
        peer: Option<String>,
        message: String,
    },
}
#[derive(Clone, Serialize, PartialEq, Eq)]
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
    fn from(e: nooboard_storage::HistoryEntry) -> Self {
        Self {
            id: e.id,
            text: e.text,
            source: e.source,
            copied_at_ms: e.copied_at_ms,
        }
    }
}
