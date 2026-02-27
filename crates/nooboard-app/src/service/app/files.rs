use nooboard_sync::{FileDecisionInput, SendFileRequest as SyncSendFileRequest};

use crate::AppResult;

use super::{AppServiceImpl, FileDecisionRequest, SendFileRequest};

impl AppServiceImpl {
    pub(super) async fn send_file_usecase(&self, request: SendFileRequest) -> AppResult<()> {
        if !request.targets.should_send() {
            return Ok(());
        }

        let sync_request = SyncSendFileRequest {
            path: request.path,
            targets: request.targets.to_sync_targets(),
        };
        let runtime = self.sync_runtime.lock().await;
        runtime.send_file(sync_request).await
    }

    pub(super) async fn respond_file_decision_usecase(
        &self,
        request: FileDecisionRequest,
    ) -> AppResult<()> {
        let input = FileDecisionInput {
            peer_node_id: request.peer_node_id.as_str().to_string(),
            transfer_id: request.transfer_id,
            accept: request.accept,
            reason: request.reason,
        };
        let runtime = self.sync_runtime.lock().await;
        runtime.send_file_decision(input).await
    }
}
