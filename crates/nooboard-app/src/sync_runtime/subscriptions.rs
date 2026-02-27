use tokio::sync::broadcast;

use nooboard_sync::{SyncEvent, TransferUpdate};

use super::SyncRuntime;

impl SyncRuntime {
    pub fn subscribe_events(&self) -> broadcast::Receiver<SyncEvent> {
        self.state.event_tx.subscribe()
    }

    pub fn subscribe_transfer_updates(&self) -> broadcast::Receiver<TransferUpdate> {
        self.state.transfer_tx.subscribe()
    }
}
