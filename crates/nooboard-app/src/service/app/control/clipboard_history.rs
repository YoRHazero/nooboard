use nooboard_sync::SendTextRequest;

use crate::service::types::{
    EventId, HistoryCursor, HistoryPage, HistoryRecord, ListHistoryRequest,
    LocalClipboardChangeRequest, LocalClipboardChangeResult, RebroadcastHistoryRequest,
    RemoteTextRequest, find_recent_record, now_millis_i64,
};
use crate::{AppError, AppResult};

use super::state::ControlState;

pub(super) async fn apply_local_clipboard_change(
    state: &ControlState,
    request: LocalClipboardChangeRequest,
) -> AppResult<LocalClipboardChangeResult> {
    let event_id = EventId::new();
    let now_ms = now_millis_i64();
    let broadcast_attempted = state.config.sync.network.enabled && request.targets.should_send();

    if broadcast_attempted {
        let _ = state
            .storage_runtime
            .append_text_with_outbox(
                &request.text,
                event_id.as_uuid(),
                Some(state.config.identity.device_id.as_str()),
                now_ms,
                now_ms,
                request.targets.to_sync_targets(),
                now_ms,
            )
            .await?;
    } else {
        let _ = state
            .storage_runtime
            .append_text(
                &request.text,
                Some(event_id.as_uuid()),
                Some(state.config.identity.device_id.as_str()),
                now_ms,
                now_ms,
            )
            .await?;
    }

    Ok(LocalClipboardChangeResult {
        event_id,
        broadcast_attempted,
    })
}

pub(super) async fn apply_history_entry_to_clipboard(
    state: &ControlState,
    event_id: EventId,
) -> AppResult<()> {
    let recent_limit = state.config.recent_event_lookup_limit();
    let records = state
        .storage_runtime
        .list_history(recent_limit, None)
        .await?;
    let record = find_recent_record(records, event_id, recent_limit)?;
    state.clipboard.write_text(&record.content)
}

pub(super) async fn list_history(
    state: &ControlState,
    request: ListHistoryRequest,
) -> AppResult<HistoryPage> {
    let storage_cursor = request
        .cursor
        .as_ref()
        .map(HistoryCursor::to_storage_cursor);
    let records = state
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

pub(super) async fn rebroadcast_history_entry(
    state: &ControlState,
    request: RebroadcastHistoryRequest,
) -> AppResult<()> {
    if !request.targets.should_send() {
        return Ok(());
    }

    if !state.config.sync.network.enabled {
        return Err(AppError::SyncDisabled);
    }

    let recent_limit = state.config.recent_event_lookup_limit();
    let records = state
        .storage_runtime
        .list_history(recent_limit, None)
        .await?;
    let record = find_recent_record(records, request.event_id, recent_limit)?;

    let sync_request = SendTextRequest {
        event_id: uuid::Uuid::from_bytes(record.event_id).to_string(),
        content: record.content,
        targets: request.targets.to_sync_targets(),
    };
    let text_tx = state.sync_runtime.text_sender()?;
    text_tx
        .send(sync_request)
        .await
        .map_err(|error| AppError::ChannelClosed(format!("sync text_tx closed: {error}")))?;
    Ok(())
}

pub(super) async fn store_remote_text(
    state: &ControlState,
    request: RemoteTextRequest,
) -> AppResult<()> {
    let now_ms = now_millis_i64();
    let _ = state
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

pub(super) async fn write_remote_text_to_clipboard(
    state: &ControlState,
    request: RemoteTextRequest,
) -> AppResult<()> {
    state.clipboard.write_text(&request.content)
}
