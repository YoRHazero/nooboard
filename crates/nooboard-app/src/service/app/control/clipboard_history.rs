use nooboard_sync::SendTextRequest;
use tokio::sync::mpsc::error::TrySendError;

use crate::service::types::{
    AppEvent, ClipboardBroadcastTargets, ClipboardHistoryCursor, ClipboardHistoryPage,
    ClipboardRecordSource, EventId, ListClipboardHistoryRequest, NoobId,
    RebroadcastClipboardRequest, SubmitTextRequest, now_millis_i64,
};
use crate::{AppError, AppResult};

use super::state::ControlState;

pub(super) async fn submit_text(
    state: &mut ControlState,
    request: SubmitTextRequest,
) -> AppResult<EventId> {
    let event_id = EventId::new();
    let local_noob_id = state.app_state.identity.noob_id.clone();
    let local_device_id = state.app_state.identity.device_id.clone();
    commit_clipboard_record(
        state,
        event_id,
        request.content,
        local_noob_id,
        local_device_id,
        ClipboardRecordSource::UserSubmit,
    )
    .await?;
    Ok(event_id)
}

pub(super) async fn get_clipboard_record(
    state: &ControlState,
    event_id: EventId,
) -> AppResult<crate::service::types::ClipboardRecord> {
    let record = load_record_by_event_id(state, event_id).await?;
    Ok(map_record(record))
}

pub(super) async fn list_clipboard_history(
    state: &ControlState,
    request: ListClipboardHistoryRequest,
) -> AppResult<ClipboardHistoryPage> {
    let storage_cursor = request
        .cursor
        .as_ref()
        .map(ClipboardHistoryCursor::to_storage_cursor);
    let records = state
        .storage_runtime
        .list_history(request.limit, storage_cursor)
        .await?;
    let next_cursor = records.last().map(ClipboardHistoryCursor::from);
    let records = records.into_iter().map(map_record).collect();
    Ok(ClipboardHistoryPage {
        records,
        next_cursor,
    })
}

pub(super) async fn adopt_clipboard_record(
    state: &ControlState,
    event_id: EventId,
) -> AppResult<()> {
    let record = load_record_by_event_id(state, event_id).await?;
    state
        .clipboard
        .write_text_with_event(event_id, &record.content)
}

pub(super) async fn rebroadcast_clipboard_record(
    state: &ControlState,
    request: RebroadcastClipboardRequest,
) -> AppResult<()> {
    if !state.config.sync.network.enabled {
        return Err(AppError::SyncDisabled);
    }

    let record = load_record_by_event_id(state, request.event_id).await?;
    let targets = resolve_broadcast_targets(state, &request.targets)?;
    let sync_request = SendTextRequest {
        event_id: request.event_id.to_string(),
        content: record.content,
        targets,
    };

    let text_tx = state.sync_runtime.text_sender()?;
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

pub(super) async fn commit_local_capture(
    state: &mut ControlState,
    event_id: EventId,
    content: String,
) -> AppResult<()> {
    let local_noob_id = state.app_state.identity.noob_id.clone();
    let local_device_id = state.app_state.identity.device_id.clone();
    let _ = commit_clipboard_record(
        state,
        event_id,
        content,
        local_noob_id,
        local_device_id,
        ClipboardRecordSource::LocalCapture,
    )
    .await?;
    Ok(())
}

pub(super) async fn commit_remote_sync(
    state: &mut ControlState,
    event_id: EventId,
    content: String,
    origin_noob_id: NoobId,
    origin_device_id: String,
) -> AppResult<()> {
    let _ = commit_clipboard_record(
        state,
        event_id,
        content,
        origin_noob_id,
        origin_device_id,
        ClipboardRecordSource::RemoteSync,
    )
    .await?;
    Ok(())
}

async fn commit_clipboard_record(
    state: &mut ControlState,
    event_id: EventId,
    content: String,
    origin_noob_id: NoobId,
    origin_device_id: String,
    source: ClipboardRecordSource,
) -> AppResult<bool> {
    validate_text_size(state, &content)?;

    let now_ms = now_millis_i64();
    let inserted = state
        .storage_runtime
        .append_text_with_source(
            &content,
            Some(event_id.as_uuid()),
            Some(origin_noob_id.as_str()),
            Some(origin_device_id.as_str()),
            now_ms,
            now_ms,
            map_record_source_to_storage(source),
        )
        .await?;
    if !inserted {
        return Ok(false);
    }

    state.update_state(|app_state| {
        app_state.clipboard.latest_committed_event_id = Some(event_id);
    });
    state.publish_event(AppEvent::ClipboardCommitted { event_id, source });
    Ok(true)
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

fn map_record(record: nooboard_storage::HistoryRecord) -> crate::service::types::ClipboardRecord {
    let source = map_storage_source(record.source);
    crate::service::types::ClipboardRecord::from_storage(record, source)
}

fn map_record_source_to_storage(
    source: ClipboardRecordSource,
) -> nooboard_storage::HistoryRecordSource {
    match source {
        ClipboardRecordSource::LocalCapture => nooboard_storage::HistoryRecordSource::LocalCapture,
        ClipboardRecordSource::RemoteSync => nooboard_storage::HistoryRecordSource::RemoteSync,
        ClipboardRecordSource::UserSubmit => nooboard_storage::HistoryRecordSource::UserSubmit,
    }
}

fn map_storage_source(source: nooboard_storage::HistoryRecordSource) -> ClipboardRecordSource {
    match source {
        nooboard_storage::HistoryRecordSource::LocalCapture => ClipboardRecordSource::LocalCapture,
        nooboard_storage::HistoryRecordSource::RemoteSync => ClipboardRecordSource::RemoteSync,
        nooboard_storage::HistoryRecordSource::UserSubmit => ClipboardRecordSource::UserSubmit,
    }
}

fn resolve_broadcast_targets(
    state: &ControlState,
    targets: &ClipboardBroadcastTargets,
) -> AppResult<Option<Vec<String>>> {
    let connected: Vec<String> = state
        .app_state
        .peers
        .connected
        .iter()
        .map(|peer| peer.noob_id.as_str().to_string())
        .collect();

    match targets {
        ClipboardBroadcastTargets::AllConnected => {
            if connected.is_empty() {
                return Err(AppError::PeerNotConnected {
                    peer_noob_id: "<all-connected>".to_string(),
                });
            }
            Ok(None)
        }
        ClipboardBroadcastTargets::Nodes(nodes) => {
            for node in nodes {
                if !connected.iter().any(|peer| peer == node.as_str()) {
                    return Err(AppError::PeerNotConnected {
                        peer_noob_id: node.as_str().to_string(),
                    });
                }
            }
            Ok(Some(
                nodes
                    .iter()
                    .map(|node| node.as_str().to_string())
                    .collect::<Vec<_>>(),
            ))
        }
    }
}

fn validate_text_size(state: &ControlState, content: &str) -> AppResult<()> {
    let actual_bytes = content.len();
    let max_bytes = state.config.storage.max_text_bytes;
    if actual_bytes > max_bytes {
        return Err(AppError::TextTooLarge {
            actual_bytes,
            max_bytes,
        });
    }
    Ok(())
}
