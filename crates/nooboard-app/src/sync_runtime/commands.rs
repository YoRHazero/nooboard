use tokio::sync::mpsc;

use nooboard_sync::{FileDecisionInput, SendFileRequest, SendTextRequest, SyncControlCommand};

use crate::{AppError, AppResult};

use super::SyncRuntime;

impl SyncRuntime {
    pub fn text_sender(&self) -> AppResult<mpsc::Sender<SendTextRequest>> {
        self.state
            .engine
            .as_ref()
            .map(|engine| engine.text_tx.clone())
            .ok_or(AppError::EngineNotRunning)
    }

    pub fn file_sender(&self) -> AppResult<mpsc::Sender<SendFileRequest>> {
        self.state
            .engine
            .as_ref()
            .map(|engine| engine.file_tx.clone())
            .ok_or(AppError::EngineNotRunning)
    }

    pub fn decision_sender(&self) -> AppResult<mpsc::Sender<FileDecisionInput>> {
        self.state
            .engine
            .as_ref()
            .map(|engine| engine.decision_tx.clone())
            .ok_or(AppError::EngineNotRunning)
    }

    pub fn control_sender(&self) -> AppResult<mpsc::Sender<SyncControlCommand>> {
        self.state
            .engine
            .as_ref()
            .map(|engine| engine.control_tx.clone())
            .ok_or(AppError::EngineNotRunning)
    }
}
