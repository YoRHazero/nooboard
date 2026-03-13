use std::fmt::{Display, Formatter};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::errors::NetworkEventRecvError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SessionId(Uuid);

impl SessionId {
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }
}

impl Display for SessionId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for SessionId {
    type Err = uuid::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(value).map(Self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct DirectSeedId(Uuid);

impl DirectSeedId {
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    pub fn from_uuid(value: Uuid) -> Self {
        Self(value)
    }

    pub fn as_uuid(self) -> Uuid {
        self.0
    }
}

impl Display for DirectSeedId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for DirectSeedId {
    type Err = uuid::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(value).map(Self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct DirectRequestId(Uuid);

impl DirectRequestId {
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }
}

impl Display for DirectRequestId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TransferTicket {
    pub session_id: SessionId,
    pub raw_id: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionMode {
    Lan,
    Direct,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetworkStatus {
    Stopped,
    Starting,
    Running,
    Error(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkSnapshot {
    pub status: NetworkStatus,
    pub lan_enabled: bool,
    pub lan_peers: Vec<LanPeerInfo>,
    pub direct_seeds: Vec<DirectSeedInfo>,
    pub pending_direct_requests: Vec<PendingDirectRequest>,
    pub sessions: Vec<SessionInfo>,
    pub transfers: TransfersSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanPeerInfo {
    pub noob_id: String,
    pub device_id: String,
    pub addresses: Vec<SocketAddr>,
    pub last_seen_at_ms: u64,
    pub connected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectSeedInfo {
    pub id: DirectSeedId,
    pub label: String,
    pub host: String,
    pub port: u16,
    pub enabled: bool,
    pub learned_device_id: Option<String>,
    pub last_connected_addr: Option<SocketAddr>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingDirectRequest {
    pub id: DirectRequestId,
    pub remote_addr: SocketAddr,
    pub peer_noob_id: String,
    pub peer_device_id: String,
    pub expires_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionInfo {
    pub id: SessionId,
    pub mode: ConnectionMode,
    pub peer_noob_id: String,
    pub peer_device_id: String,
    pub remote_addr: SocketAddr,
    pub local_bind_addr: Option<SocketAddr>,
    pub outbound: bool,
    pub connected_at_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferDirection {
    Upload,
    Download,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveTransferState {
    Queued,
    Starting,
    InProgress,
    Cancelling,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferOutcome {
    Succeeded,
    Rejected,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncomingTransferOffer {
    pub ticket: TransferTicket,
    pub session_id: SessionId,
    pub peer_noob_id: String,
    pub peer_device_id: String,
    pub file_name: String,
    pub file_size: u64,
    pub total_chunks: u32,
    pub offered_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveTransferInfo {
    pub ticket: TransferTicket,
    pub session_id: SessionId,
    pub peer_noob_id: String,
    pub peer_device_id: String,
    pub file_name: String,
    pub file_size: u64,
    pub transferred_bytes: u64,
    pub direction: TransferDirection,
    pub state: ActiveTransferState,
    pub updated_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletedTransferInfo {
    pub ticket: TransferTicket,
    pub session_id: SessionId,
    pub peer_noob_id: String,
    pub peer_device_id: String,
    pub file_name: String,
    pub file_size: u64,
    pub direction: TransferDirection,
    pub outcome: TransferOutcome,
    pub saved_path: Option<PathBuf>,
    pub message: Option<String>,
    pub finished_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TransfersSnapshot {
    pub incoming_pending: Vec<IncomingTransferOffer>,
    pub active: Vec<ActiveTransferInfo>,
    pub recent_completed: Vec<CompletedTransferInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionFailureKind {
    ResolveFailed,
    ConnectFailed,
    TlsHandshakeFailed,
    ProtocolMismatch,
    AuthRejected,
    DirectRejected,
    DirectExpired,
    AlreadyConnected,
    Io,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionFailure {
    pub kind: ConnectionFailureKind,
    pub mode: ConnectionMode,
    pub peer_noob_id: Option<String>,
    pub peer_device_id: Option<String>,
    pub remote_addr: Option<SocketAddr>,
    pub local_bind_addr: Option<SocketAddr>,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetworkEvent {
    StatusChanged(NetworkStatus),
    LanPeersChanged,
    DirectSeedsChanged,
    PendingDirectRequestsChanged,
    SessionsChanged,
    ConnectionFailed(ConnectionFailure),
    TextReceived {
        session_id: SessionId,
        event_id: String,
        content: String,
        peer_noob_id: String,
        peer_device_id: String,
    },
    IncomingTransferOffered {
        offer: IncomingTransferOffer,
    },
    TransferUpdated {
        transfer: ActiveTransferInfo,
    },
    TransferCompleted {
        transfer: CompletedTransferInfo,
    },
}

pub struct NetworkSubscription {
    receiver: broadcast::Receiver<NetworkEvent>,
}

impl NetworkSubscription {
    pub(crate) fn new(receiver: broadcast::Receiver<NetworkEvent>) -> Self {
        Self { receiver }
    }

    pub async fn recv(&mut self) -> Result<NetworkEvent, NetworkEventRecvError> {
        self.receiver
            .recv()
            .await
            .map_err(|_| NetworkEventRecvError::Closed)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpsertDirectSeedInput {
    pub id: Option<DirectSeedId>,
    pub label: String,
    pub host: String,
    pub port: u16,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectDirectOutcome {
    Started,
    AlreadyConnected(SessionId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionTarget {
    AllConnected,
    Sessions(Vec<SessionId>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendTextRequest {
    pub event_id: String,
    pub content: String,
    pub target: SessionTarget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendFilesRequest {
    pub files: Vec<PathBuf>,
    pub target: SessionTarget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IncomingTransferDisposition {
    Accept,
    Reject,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncomingTransferDecision {
    pub ticket: TransferTicket,
    pub decision: IncomingTransferDisposition,
}
