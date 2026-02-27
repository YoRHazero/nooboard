use tokio::sync::{broadcast, watch};

use nooboard_sync::{SyncEvent, SyncStatus, TransferUpdate};

use crate::{AppError, AppResult};

use super::SyncRuntime;

impl SyncRuntime {
    pub fn subscribe_events(&self) -> AppResult<broadcast::Receiver<SyncEvent>> {
        if self.state.engine.is_some() {
            Ok(self.state.event_tx.subscribe())
        } else {
            Err(AppError::EngineNotRunning)
        }
    }

    pub fn subscribe_transfer_updates(&self) -> AppResult<broadcast::Receiver<TransferUpdate>> {
        if self.state.engine.is_some() {
            Ok(self.state.transfer_tx.subscribe())
        } else {
            Err(AppError::EngineNotRunning)
        }
    }

    pub fn subscribe_status(&self) -> AppResult<watch::Receiver<SyncStatus>> {
        self.state
            .engine
            .as_ref()
            .map(|engine| engine.status_rx.clone())
            .ok_or(AppError::EngineNotRunning)
    }
}
