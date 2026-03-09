use tokio::sync::{mpsc, oneshot};

use nooboard_sync::{
    CancelTransferRequest, FileDecisionInput, ScheduledTransfer, SendFileCommand, SendFileRequest,
    SendTextRequest, SyncControlCommand,
};

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

    pub async fn send_file(&self, request: SendFileRequest) -> AppResult<Vec<ScheduledTransfer>> {
        let file_tx = self
            .state
            .engine
            .as_ref()
            .map(|engine| engine.file_tx.clone())
            .ok_or(AppError::EngineNotRunning)?;
        let (reply_tx, reply_rx) = oneshot::channel();
        file_tx
            .send(SendFileCommand {
                request,
                reply: reply_tx,
            })
            .await
            .map_err(|error| AppError::ChannelClosed(format!("sync file_tx closed: {error}")))?;
        reply_rx
            .await
            .map_err(|_| AppError::ChannelClosed("sync file reply channel closed".to_string()))?
            .map_err(Into::into)
    }

    pub fn decision_sender(&self) -> AppResult<mpsc::Sender<FileDecisionInput>> {
        self.state
            .engine
            .as_ref()
            .map(|engine| engine.decision_tx.clone())
            .ok_or(AppError::EngineNotRunning)
    }

    pub async fn cancel_transfer(&self, request: CancelTransferRequest) -> AppResult<()> {
        let control_tx = self
            .state
            .engine
            .as_ref()
            .map(|engine| engine.control_tx.clone())
            .ok_or(AppError::EngineNotRunning)?;
        let (reply_tx, reply_rx) = oneshot::channel();
        control_tx
            .send(SyncControlCommand::CancelTransfer {
                request,
                reply: reply_tx,
            })
            .await
            .map_err(|error| AppError::ChannelClosed(format!("sync control_tx closed: {error}")))?;
        reply_rx
            .await
            .map_err(|_| AppError::ChannelClosed("sync cancel reply channel closed".to_string()))?
            .map_err(Into::into)
    }
}
