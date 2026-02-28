use nooboard_sync::SendTextRequest;
use tokio::sync::mpsc::error::TrySendError;

use crate::service::types::{
    BroadcastDropReason, BroadcastStatus, EventId, HistoryCursor, HistoryPage, HistoryRecord,
    ListHistoryRequest, LocalClipboardChangeRequest, LocalClipboardChangeResult,
    RebroadcastHistoryRequest, RemoteTextRequest, find_recent_record, now_millis_i64,
};
use crate::{AppError, AppResult};

use super::state::ControlState;

pub(super) async fn apply_local_clipboard_change(
    state: &ControlState,
    request: LocalClipboardChangeRequest,
) -> AppResult<LocalClipboardChangeResult> {
    let LocalClipboardChangeRequest { text, targets } = request;
    let event_id = EventId::new();
    let now_ms = now_millis_i64();

    let _ = state
        .storage_runtime
        .append_text(
            &text,
            Some(event_id.as_uuid()),
            Some(state.config.identity.device_id.as_str()),
            now_ms,
            now_ms,
        )
        .await?;

    let broadcast_status = if !targets.should_send() {
        BroadcastStatus::NotRequested
    } else if !state.config.sync.network.enabled {
        BroadcastStatus::Dropped(BroadcastDropReason::NetworkDisabled)
    } else {
        try_send_sync_text_best_effort(
            state,
            SendTextRequest {
                event_id: event_id.as_uuid().to_string(),
                content: text,
                targets: targets.to_sync_targets(),
            },
        )
    };

    Ok(LocalClipboardChangeResult {
        event_id,
        broadcast_status,
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
    try_send_sync_text_strict(state, sync_request)
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

fn try_send_sync_text_best_effort(
    state: &ControlState,
    sync_request: SendTextRequest,
) -> BroadcastStatus {
    let text_tx = match state.sync_runtime.text_sender() {
        Ok(tx) => tx,
        Err(AppError::EngineNotRunning) => {
            return BroadcastStatus::Dropped(BroadcastDropReason::EngineNotRunning);
        }
        Err(_) => {
            return BroadcastStatus::Dropped(BroadcastDropReason::QueueClosed);
        }
    };

    let connected_peer_ids = state
        .sync_runtime
        .connected_peers()
        .into_iter()
        .map(|peer| peer.peer_node_id)
        .collect::<Vec<_>>();
    if !has_eligible_peer(&connected_peer_ids, sync_request.targets.as_deref()) {
        return BroadcastStatus::Dropped(BroadcastDropReason::NoEligiblePeer);
    }

    match text_tx.try_send(sync_request) {
        Ok(()) => BroadcastStatus::Sent,
        Err(TrySendError::Full(_)) => BroadcastStatus::Dropped(BroadcastDropReason::QueueFull),
        Err(TrySendError::Closed(_)) => BroadcastStatus::Dropped(BroadcastDropReason::QueueClosed),
    }
}

fn try_send_sync_text_strict(state: &ControlState, sync_request: SendTextRequest) -> AppResult<()> {
    match try_send_sync_text_best_effort(state, sync_request) {
        BroadcastStatus::Sent => Ok(()),
        BroadcastStatus::Dropped(BroadcastDropReason::NoEligiblePeer) => Ok(()),
        BroadcastStatus::Dropped(BroadcastDropReason::EngineNotRunning) => {
            Err(AppError::EngineNotRunning)
        }
        BroadcastStatus::Dropped(BroadcastDropReason::QueueFull) => Err(AppError::ChannelClosed(
            "sync text_tx queue is full".to_string(),
        )),
        BroadcastStatus::Dropped(BroadcastDropReason::QueueClosed) => Err(AppError::ChannelClosed(
            "sync text_tx is closed".to_string(),
        )),
        BroadcastStatus::Dropped(BroadcastDropReason::NetworkDisabled)
        | BroadcastStatus::NotRequested => Ok(()),
    }
}

fn has_eligible_peer(connected_peer_ids: &[String], targets: Option<&[String]>) -> bool {
    if connected_peer_ids.is_empty() {
        return false;
    }

    match targets {
        None => true,
        Some(targets) => targets.iter().any(|target| {
            connected_peer_ids
                .iter()
                .any(|peer_id| peer_id == target.trim())
        }),
    }
}
