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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncEvent {
    TextReceived(String),
    FileDownloaded {
        path: PathBuf,
        size: u64,
    },
    FileDecisionRequired {
        peer_node_id: String,
        transfer_id: u32,
        file_name: String,
        file_size: u64,
        total_chunks: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDecisionInput {
    pub peer_node_id: String,
    pub transfer_id: u32,
    pub accept: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncControlCommand {
    DisconnectPeer { peer_node_id: String },
}

pub struct SyncEngineHandle {
    pub text_tx: mpsc::Sender<String>,
    pub file_tx: mpsc::Sender<PathBuf>,
    pub decision_tx: mpsc::Sender<FileDecisionInput>,
    pub control_tx: mpsc::Sender<SyncControlCommand>,
    pub event_rx: mpsc::Receiver<SyncEvent>,
    pub status_rx: watch::Receiver<SyncStatus>,
    pub shutdown_tx: broadcast::Sender<()>,
}
