use std::net::SocketAddr;
use std::path::PathBuf;

use tokio::sync::{broadcast, mpsc, watch};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncStatus {
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
pub struct ConnectedPeerInfo {
    pub peer_node_id: String,
    pub addr: SocketAddr,
    pub outbound: bool,
    pub connected_at_ms: u64,
    pub state: PeerConnectionState,
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
pub enum TransferDirection {
    Incoming,
    Outgoing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferUpdate {
    pub transfer_id: u32,
    pub peer_node_id: String,
    pub direction: TransferDirection,
    pub state: TransferState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncEvent {
    TextReceived(String),
    FileDecisionRequired {
        peer_node_id: String,
        transfer_id: u32,
        file_name: String,
        file_size: u64,
        total_chunks: u32,
    },
    ConnectionError {
        peer_node_id: Option<String>,
        addr: Option<SocketAddr>,
        error: String,
    },
    TransferUpdate(TransferUpdate),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDecisionInput {
    pub peer_node_id: String,
    pub transfer_id: u32,
    pub accept: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendTextRequest {
    pub event_id: String,
    pub content: String,
    pub targets: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendFileRequest {
    pub path: PathBuf,
    pub targets: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncControlCommand {
    DisconnectPeer { peer_node_id: String },
}

pub struct SyncEngineHandle {
    pub text_tx: mpsc::Sender<SendTextRequest>,
    pub file_tx: mpsc::Sender<SendFileRequest>,
    pub decision_tx: mpsc::Sender<FileDecisionInput>,
    pub control_tx: mpsc::Sender<SyncControlCommand>,
    pub event_rx: mpsc::Receiver<SyncEvent>,
    pub progress_rx: broadcast::Receiver<TransferUpdate>,
    pub peers_rx: watch::Receiver<Vec<ConnectedPeerInfo>>,
    pub status_rx: watch::Receiver<SyncStatus>,
    pub shutdown_tx: broadcast::Sender<()>,
}
