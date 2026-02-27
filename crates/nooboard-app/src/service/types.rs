use std::collections::HashSet;
use std::fmt::{Display, Formatter};
use std::net::SocketAddr;
use std::path::PathBuf;

use nooboard_storage::HistoryCursor as StorageHistoryCursor;
use uuid::Uuid;

use crate::{AppError, AppResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EventId(Uuid);

impl EventId {
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    pub fn as_uuid(self) -> Uuid {
        self.0
    }
}

impl Display for EventId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl From<Uuid> for EventId {
    fn from(value: Uuid) -> Self {
        Self(value)
    }
}

impl TryFrom<&str> for EventId {
    type Error = AppError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Uuid::parse_str(value)
            .map(Self)
            .map_err(|_| AppError::InvalidEventId {
                event_id: value.to_string(),
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NodeId(String);

impl NodeId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Targets {
    #[default]
    All,
    Nodes(Vec<NodeId>),
}

impl Targets {
    pub fn all() -> Self {
        Self::All
    }

    pub fn nodes(nodes: Vec<NodeId>) -> Self {
        Self::Nodes(nodes)
    }

    pub(crate) fn should_send(&self) -> bool {
        match self {
            Self::All => true,
            Self::Nodes(nodes) => nodes.iter().any(|node| !node.as_str().trim().is_empty()),
        }
    }

    pub(crate) fn to_sync_targets(&self) -> Option<Vec<String>> {
        match self {
            Self::All => None,
            Self::Nodes(nodes) => {
                let mut seen = HashSet::new();
                let normalized: Vec<String> = nodes
                    .iter()
                    .map(|node| node.as_str().trim().to_string())
                    .filter(|node| !node.is_empty())
                    .filter(|node| seen.insert(node.clone()))
                    .collect();
                Some(normalized)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryRecord {
    pub event_id: EventId,
    pub origin_device_id: String,
    pub created_at_ms: i64,
    pub applied_at_ms: i64,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryCursor {
    pub created_at_ms: i64,
    pub event_id: EventId,
}

impl HistoryCursor {
    pub(crate) fn to_storage_cursor(&self) -> StorageHistoryCursor {
        StorageHistoryCursor {
            created_at_ms: self.created_at_ms,
            event_id: *self.event_id.as_uuid().as_bytes(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryPage {
    pub records: Vec<HistoryRecord>,
    pub next_cursor: Option<HistoryCursor>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListHistoryRequest {
    pub limit: usize,
    pub cursor: Option<HistoryCursor>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalClipboardChangeRequest {
    pub text: String,
    pub targets: Targets,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalClipboardChangeResult {
    pub event_id: EventId,
    pub broadcast_attempted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RebroadcastHistoryRequest {
    pub event_id: EventId,
    pub targets: Targets,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteTextRequest {
    pub event_id: EventId,
    pub content: String,
    pub device_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendFileRequest {
    pub path: PathBuf,
    pub targets: Targets,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDecisionRequest {
    pub peer_node_id: NodeId,
    pub transfer_id: u32,
    pub accept: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetworkPatch {
    SetMdnsEnabled(bool),
    SetNetworkEnabled(bool),
    AddManualPeer(SocketAddr),
    RemoveManualPeer(SocketAddr),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BroadcastConfig {
    pub network_enabled: bool,
    pub mdns_enabled: bool,
    pub manual_peers: Vec<SocketAddr>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppSyncStatus {
    Disabled,
    Starting,
    Running,
    Stopped,
    Error(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerConnectionState {
    Connected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectedPeer {
    pub peer_node_id: NodeId,
    pub peer_device_id: String,
    pub addr: SocketAddr,
    pub outbound: bool,
    pub connected_at_ms: u64,
    pub state: PeerConnectionState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransferDirection {
    Incoming,
    Outgoing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransferState {
    Started {
        file_name: String,
        total_bytes: u64,
    },
    Progress {
        done_bytes: u64,
        total_bytes: u64,
        bps: Option<u64>,
        eta_ms: Option<u64>,
    },
    Finished {
        path: Option<PathBuf>,
    },
    Failed {
        reason: String,
    },
    Cancelled {
        reason: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferUpdate {
    pub transfer_id: u32,
    pub peer_node_id: NodeId,
    pub direction: TransferDirection,
    pub state: TransferState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncEvent {
    TextReceived {
        event_id: EventId,
        content: String,
        device_id: String,
    },
    FileDecisionRequired {
        peer_node_id: NodeId,
        transfer_id: u32,
        file_name: String,
        file_size: u64,
        total_chunks: u32,
    },
    ConnectionError {
        peer_node_id: Option<NodeId>,
        addr: Option<SocketAddr>,
        error: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppEvent {
    Sync(SyncEvent),
    Transfer(TransferUpdate),
}

pub(crate) fn now_millis_i64() -> i64 {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);

    if millis > i64::MAX as u128 {
        i64::MAX
    } else {
        millis as i64
    }
}

pub(crate) fn find_recent_record(
    records: Vec<nooboard_storage::HistoryRecord>,
    event_id: EventId,
    recent_limit: usize,
) -> AppResult<nooboard_storage::HistoryRecord> {
    let target = *event_id.as_uuid().as_bytes();
    records
        .into_iter()
        .find(|record| record.event_id == target)
        .ok_or(AppError::NotFoundInRecentWindow {
            event_id: event_id.to_string(),
            limit: recent_limit,
        })
}
