use tokio::sync::{broadcast, mpsc, watch};
use tokio::task::JoinHandle;

use nooboard_sync::{
    ConnectedPeerInfo, FileDecisionInput, SendFileRequest, SendTextRequest, SyncControlCommand,
    SyncEvent, SyncStatus, TransferUpdate,
};

pub(super) const BRIDGE_CHANNEL_CAPACITY: usize = 256;

pub(super) struct RuntimeState {
    pub(super) engine: Option<RunningEngine>,
    pub(super) event_tx: broadcast::Sender<SyncEvent>,
    pub(super) transfer_tx: broadcast::Sender<TransferUpdate>,
}

impl RuntimeState {
    pub(super) fn new() -> Self {
        let (event_tx, _) = broadcast::channel(BRIDGE_CHANNEL_CAPACITY);
        let (transfer_tx, _) = broadcast::channel(BRIDGE_CHANNEL_CAPACITY);
        Self {
            engine: None,
            event_tx,
            transfer_tx,
        }
    }
}

pub(super) struct RunningEngine {
    pub(super) text_tx: mpsc::Sender<SendTextRequest>,
    pub(super) file_tx: mpsc::Sender<SendFileRequest>,
    pub(super) decision_tx: mpsc::Sender<FileDecisionInput>,
    pub(super) control_tx: mpsc::Sender<SyncControlCommand>,
    pub(super) peers_rx: watch::Receiver<Vec<ConnectedPeerInfo>>,
    pub(super) status_rx: watch::Receiver<SyncStatus>,
    pub(super) shutdown_tx: broadcast::Sender<()>,
    pub(super) event_task: JoinHandle<()>,
    pub(super) transfer_task: JoinHandle<()>,
}
