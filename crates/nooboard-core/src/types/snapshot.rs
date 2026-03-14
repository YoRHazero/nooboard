use std::fmt::{Display, Formatter};
use std::net::SocketAddr;
use std::str::FromStr;

use tokio::sync::watch;
use uuid::Uuid;

use super::settings::WorkspaceSettings;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceSnapshot {
    pub revision: u64,
    pub identity: WorkspaceIdentity,
    pub local_connection: LocalConnectionInfo,
    pub clipboard: ClipboardState,
    pub settings: WorkspaceSettings,
    pub network: nooboard_network::NetworkSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceIdentity {
    pub noob_id: NoobId,
    pub device_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LocalConnectionInfo {
    pub device_endpoint: Option<SocketAddr>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ClipboardState {
    pub latest_committed_event_id: Option<EventId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EventId(Uuid);

impl EventId {
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    pub fn as_uuid(self) -> Uuid {
        self.0
    }
}

impl From<Uuid> for EventId {
    fn from(value: Uuid) -> Self {
        Self(value)
    }
}

impl Display for EventId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for EventId {
    type Err = uuid::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(value).map(Self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NoobId(String);

impl NoobId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl Display for NoobId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

pub type StateRecvError = watch::error::RecvError;

pub struct StateSubscription {
    latest: WorkspaceSnapshot,
    receiver: watch::Receiver<WorkspaceSnapshot>,
}

impl StateSubscription {
    pub(crate) fn new(receiver: watch::Receiver<WorkspaceSnapshot>) -> Self {
        let latest = receiver.borrow().clone();
        Self { latest, receiver }
    }

    pub async fn recv(&mut self) -> Result<WorkspaceSnapshot, StateRecvError> {
        self.receiver.changed().await?;
        self.latest = self.receiver.borrow().clone();
        Ok(self.latest.clone())
    }

    pub fn latest(&self) -> &WorkspaceSnapshot {
        &self.latest
    }
}
