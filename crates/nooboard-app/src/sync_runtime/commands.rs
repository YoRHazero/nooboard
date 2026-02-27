use nooboard_sync::{FileDecisionInput, SendFileRequest, SendTextRequest, SyncControlCommand};

use crate::{AppError, AppResult};

use super::SyncRuntime;

impl SyncRuntime {
    pub async fn send_text(&self, request: SendTextRequest) -> AppResult<()> {
        let engine = self
            .state
            .engine
            .as_ref()
            .ok_or(AppError::EngineNotRunning)?;
        engine
            .text_tx
            .send(request)
            .await
            .map_err(|error| AppError::ChannelClosed(format!("sync text_tx closed: {error}")))?;
        Ok(())
    }

    pub async fn send_file(&self, request: SendFileRequest) -> AppResult<()> {
        let engine = self
            .state
            .engine
            .as_ref()
            .ok_or(AppError::EngineNotRunning)?;
        engine
            .file_tx
            .send(request)
            .await
            .map_err(|error| AppError::ChannelClosed(format!("sync file_tx closed: {error}")))?;
        Ok(())
    }

    pub async fn send_file_decision(&self, input: FileDecisionInput) -> AppResult<()> {
        let engine = self
            .state
            .engine
            .as_ref()
            .ok_or(AppError::EngineNotRunning)?;
        engine.decision_tx.send(input).await.map_err(|error| {
            AppError::ChannelClosed(format!("sync decision_tx closed: {error}"))
        })?;
        Ok(())
    }

    pub async fn send_control_command(&self, command: SyncControlCommand) -> AppResult<()> {
        let engine = self
            .state
            .engine
            .as_ref()
            .ok_or(AppError::EngineNotRunning)?;
        engine
            .control_tx
            .send(command)
            .await
            .map_err(|error| AppError::ChannelClosed(format!("sync control_tx closed: {error}")))?;
        Ok(())
    }
}
