use nooboard_sync::SendTextRequest;
use tokio::sync::mpsc::error::TrySendError;

use crate::service::types::{
    AppEvent, EventId, HistoryCursor, HistoryPage, HistoryRecord, IngestTextRequest,
    ListHistoryRequest, RebroadcastEventRequest, now_millis_i64,
};
use crate::{AppError, AppResult};

use super::state::ControlState;

pub(super) async fn ingest_text_event(
    state: &ControlState,
    request: IngestTextRequest,
) -> AppResult<()> {
    let IngestTextRequest {
        event_id,
        content,
        origin_noob_id,
        origin_device_id,
        source,
    } = request;
    let now_ms = now_millis_i64();

    let inserted = state
        .storage_runtime
        .append_text(
            &content,
            Some(event_id.as_uuid()),
            Some(origin_noob_id.as_str()),
            Some(origin_device_id.as_str()),
            now_ms,
            now_ms,
        )
        .await?;

    if inserted {
        state
            .subscriptions
            .publish_app_event(AppEvent::TextIngested {
                event_id,
                origin_noob_id,
                origin_device_id,
                source,
                created_at_ms: now_ms,
            })
            .await;
    }

    Ok(())
}

pub(super) async fn write_event_to_clipboard(
    state: &ControlState,
    event_id: EventId,
) -> AppResult<()> {
    let record = load_record_by_event_id(state, event_id).await?;
    state
        .clipboard
        .write_text_with_event(event_id, &record.content)
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

pub(super) async fn rebroadcast_event(
    state: &ControlState,
    request: RebroadcastEventRequest,
) -> AppResult<()> {
    if !request.targets.should_send() {
        return Ok(());
    }

    if !state.config.sync.network.enabled {
        return Err(AppError::SyncDisabled);
    }

    let record = load_record_by_event_id(state, request.event_id).await?;
    let sync_request = SendTextRequest {
        event_id: uuid::Uuid::from_bytes(record.event_id).to_string(),
        content: record.content,
        targets: request.targets.to_sync_targets(),
    };
    send_sync_text_strict(state, sync_request)
}

async fn load_record_by_event_id(
    state: &ControlState,
    event_id: EventId,
) -> AppResult<nooboard_storage::HistoryRecord> {
    state
        .storage_runtime
        .get_event_by_id(event_id.as_uuid())
        .await?
        .ok_or(AppError::EventNotFound {
            event_id: event_id.to_string(),
        })
}

fn send_sync_text_strict(state: &ControlState, sync_request: SendTextRequest) -> AppResult<()> {
    let text_tx = state.sync_runtime.text_sender()?;
    let connected_peer_ids = state
        .sync_runtime
        .connected_peers()
        .into_iter()
        .map(|peer| peer.peer_noob_id)
        .collect::<Vec<_>>();
    if !has_eligible_peer(&connected_peer_ids, sync_request.targets.as_deref()) {
        return Ok(());
    }

    match text_tx.try_send(sync_request) {
        Ok(()) => Ok(()),
        Err(TrySendError::Full(_)) => Err(AppError::ChannelClosed(
            "sync text_tx queue is full".to_string(),
        )),
        Err(TrySendError::Closed(_)) => Err(AppError::ChannelClosed(
            "sync text_tx is closed".to_string(),
        )),
    }
}

fn has_eligible_peer(connected_peer_noob_ids: &[String], targets: Option<&[String]>) -> bool {
    if connected_peer_noob_ids.is_empty() {
        return false;
    }

    match targets {
        None => true,
        Some(targets) => targets.iter().any(|target| {
            connected_peer_noob_ids
                .iter()
                .any(|peer_noob_id| peer_noob_id == target.trim())
        }),
    }
}
