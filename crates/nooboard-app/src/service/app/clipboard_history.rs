use nooboard_sync::SendTextRequest;

use crate::AppResult;

use super::{
    AppServiceImpl, EventId, HistoryCursor, HistoryPage, HistoryRecord, ListHistoryRequest,
    LocalClipboardChangeRequest, LocalClipboardChangeResult, RebroadcastHistoryRequest,
    RemoteTextRequest, find_recent_record, now_millis_i64,
};

impl AppServiceImpl {
    pub(super) async fn apply_local_clipboard_change_usecase(
        &self,
        request: LocalClipboardChangeRequest,
    ) -> AppResult<LocalClipboardChangeResult> {
        let config = self.config.read().await.clone();
        let event_id = EventId::new();
        let now_ms = now_millis_i64();

        let _ = self
            .storage_runtime
            .append_text(
                &request.text,
                Some(event_id.as_uuid()),
                Some(config.identity.device_id.as_str()),
                now_ms,
                now_ms,
            )
            .await?;

        let broadcast_attempted = request.targets.should_send();
        if broadcast_attempted {
            let sync_request = SendTextRequest {
                event_id: event_id.to_string(),
                content: request.text,
                targets: request.targets.to_sync_targets(),
            };
            let runtime = self.sync_runtime.lock().await;
            runtime.send_text(sync_request).await?;
        }

        Ok(LocalClipboardChangeResult {
            event_id,
            broadcast_attempted,
        })
    }

    pub(super) async fn apply_history_entry_to_clipboard_usecase(
        &self,
        event_id: EventId,
    ) -> AppResult<()> {
        let config = self.config.read().await.clone();
        let recent_limit = config.recent_event_lookup_limit();
        let records = self
            .storage_runtime
            .list_history(recent_limit, None)
            .await?;
        let record = find_recent_record(records, event_id, recent_limit)?;

        self.clipboard.write_text(&record.content)
    }

    pub(super) async fn list_history_usecase(
        &self,
        request: ListHistoryRequest,
    ) -> AppResult<HistoryPage> {
        let storage_cursor = request
            .cursor
            .as_ref()
            .map(HistoryCursor::to_storage_cursor);
        let records = self
            .storage_runtime
            .list_history(request.limit, storage_cursor)
            .await?;
        let next_cursor = records.last().map(HistoryCursor::from);
        let records = records.into_iter().map(HistoryRecord::from).collect();

        Ok(HistoryPage {
            records,
            next_cursor,
        })
    }

    pub(super) async fn rebroadcast_history_entry_usecase(
        &self,
        request: RebroadcastHistoryRequest,
    ) -> AppResult<()> {
        if !request.targets.should_send() {
            return Ok(());
        }

        let config = self.config.read().await.clone();
        let recent_limit = config.recent_event_lookup_limit();
        let records = self
            .storage_runtime
            .list_history(recent_limit, None)
            .await?;
        let record = find_recent_record(records, request.event_id, recent_limit)?;

        let sync_request = SendTextRequest {
            event_id: uuid::Uuid::from_bytes(record.event_id).to_string(),
            content: record.content,
            targets: request.targets.to_sync_targets(),
        };
        let runtime = self.sync_runtime.lock().await;
        runtime.send_text(sync_request).await
    }

    pub(super) async fn store_remote_text_usecase(
        &self,
        request: RemoteTextRequest,
    ) -> AppResult<()> {
        let now_ms = now_millis_i64();
        let _ = self
            .storage_runtime
            .append_text(
                &request.content,
                Some(request.event_id.as_uuid()),
                Some(request.device_id.as_str()),
                now_ms,
                now_ms,
            )
            .await?;

        Ok(())
    }

    pub(super) async fn write_remote_text_to_clipboard_usecase(
        &self,
        request: RemoteTextRequest,
    ) -> AppResult<()> {
        self.clipboard.write_text(&request.content)
    }
}
