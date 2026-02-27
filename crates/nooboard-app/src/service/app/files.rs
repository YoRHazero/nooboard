use nooboard_sync::{FileDecisionInput, SendFileRequest as SyncSendFileRequest};

use crate::{AppError, AppResult};

use super::{AppServiceImpl, FileDecisionRequest, SendFileRequest};

impl AppServiceImpl {
    pub(super) async fn send_file_usecase(&self, request: SendFileRequest) -> AppResult<()> {
        if !request.targets.should_send() {
            return Ok(());
        }
        let config = self.config.read().await.clone();
        if !config.sync.network.enabled {
            return Err(AppError::SyncDisabled);
        }

        let sync_request = SyncSendFileRequest {
            path: request.path,
            targets: request.targets.to_sync_targets(),
        };
        let file_tx = {
            let runtime = self.sync_runtime.lock().await;
            runtime.file_sender()?
        };
        file_tx
            .send(sync_request)
            .await
            .map_err(|error| AppError::ChannelClosed(format!("sync file_tx closed: {error}")))?;
        Ok(())
    }

    pub(super) async fn respond_file_decision_usecase(
        &self,
        request: FileDecisionRequest,
    ) -> AppResult<()> {
        let config = self.config.read().await.clone();
        if !config.sync.network.enabled {
            return Err(AppError::SyncDisabled);
        }
        let input = FileDecisionInput {
            peer_node_id: request.peer_node_id.as_str().to_string(),
            transfer_id: request.transfer_id,
            accept: request.accept,
            reason: request.reason,
        };
        let decision_tx = {
            let runtime = self.sync_runtime.lock().await;
            runtime.decision_sender()?
        };
        decision_tx.send(input).await.map_err(|error| {
            AppError::ChannelClosed(format!("sync decision_tx closed: {error}"))
        })?;
        Ok(())
    }
}
