use tokio::sync::{broadcast, watch};

use nooboard_sync::{SyncEvent, SyncStatus, TransferUpdate};

use super::SyncRuntime;

impl SyncRuntime {
    pub fn subscribe_events(&self) -> broadcast::Receiver<SyncEvent> {
        self.state.event_tx.subscribe()
    }

    pub fn subscribe_transfer_updates(&self) -> broadcast::Receiver<TransferUpdate> {
        self.state.transfer_tx.subscribe()
    }

    pub fn subscribe_status(&self) -> watch::Receiver<SyncStatus> {
        self.state.status_tx.subscribe()
    }

    pub fn subscribe_connected_peers(
        &self,
    ) -> watch::Receiver<Vec<nooboard_sync::ConnectedPeerInfo>> {
        self.state.peers_tx.subscribe()
    }
}
