use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Manual,
    Automatic,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Settings {
    pub receive_directory: Option<PathBuf>,
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
            receive_directory: None,
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
            || self
                .receive_directory
                .as_ref()
                .is_some_and(|p| !p.is_absolute())
            || self.max_history_entries > 100_000
            || self.history_days == 0
            || self.history_days > 3650
            || !valid_name(&self.device_name)
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

pub(crate) fn valid_name(name: &str) -> bool {
    !name.trim().is_empty() && name.chars().count() <= 80 && !name.chars().any(char::is_control)
}
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Peer {
    pub trusted: nooboard_network::TrustedPeer,
    pub settings: PeerSettings,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Configuration {
    pub version: u16,
    pub settings: Settings,
    pub peers: BTreeMap<String, Peer>,
    pub manual_targets: Vec<String>,
    #[serde(skip)]
    pub revision: u64,
    /// Last activation revision per target, retained across coalesced watch updates.
    #[serde(skip)]
    pub automatic_revisions: BTreeMap<String, u64>,
}
impl Configuration {
    pub fn new(settings: Settings) -> Self {
        Self {
            version: 3,
            settings,
            peers: BTreeMap::new(),
            manual_targets: vec![],
            revision: 0,
            automatic_revisions: BTreeMap::new(),
        }
    }
    pub fn validate(&self) -> Result<()> {
        if self.version != 3 || self.peers.len() > 64 {
            return Err(Error::Configuration);
        }
        self.settings.validate()?;
        for (id, peer) in &self.peers {
            if id != &peer.trusted.identity.id {
                return Err(Error::Configuration);
            }
            peer.settings.validate()?;
        }
        self.targets(&self.manual_targets)
    }
    pub fn targets(&self, targets: &[String]) -> Result<()> {
        let mut seen = std::collections::HashSet::new();
        if targets
            .iter()
            .any(|id| !self.peers.contains_key(id) || !seen.insert(id))
        {
            Err(Error::Configuration)
        } else {
            Ok(())
        }
    }
    pub fn auto_send_enabled(&self, id: &str) -> bool {
        !self.settings.paused
            && self.settings.mode == Mode::Automatic
            && self
                .peers
                .get(id)
                .is_some_and(|peer| peer.settings.auto_send)
    }
}
