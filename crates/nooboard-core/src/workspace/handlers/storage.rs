use crate::error::{CoreError, CoreResult};
use crate::types::{
    ClipboardHistoryAnchor, ClipboardHistoryPage, ClipboardRecord, ClipboardRecordSource, EventId,
    ListClipboardHistoryRequest,
};

use super::super::state::WorkspaceState;

pub(crate) async fn get_clipboard_record(
    state: &WorkspaceState,
    event_id: EventId,
) -> CoreResult<ClipboardRecord> {
    let record = load_record_by_event_id(state, event_id).await?;
    Ok(ClipboardRecord::from_storage(
        record.clone(),
        map_storage_source(record.source),
    ))
}

pub(crate) async fn list_clipboard_history(
    state: &WorkspaceState,
    request: ListClipboardHistoryRequest,
) -> CoreResult<ClipboardHistoryPage> {
    let storage_anchor = request
        .anchor
        .as_ref()
        .map(ClipboardHistoryAnchor::to_storage_anchor);
    let page = state
        .storage_runtime()
        .list_history(nooboard_storage::ListHistoryRequest {
            limit: request.limit,
            direction: request.direction.to_storage_direction(),
            anchor: storage_anchor,
        })
        .await?;
    let next_anchor = page.next_anchor.map(|anchor| ClipboardHistoryAnchor {
        created_at_ms: anchor.created_at_ms,
        event_id: EventId::from(uuid::Uuid::from_bytes(anchor.event_id)),
    });
    let records = page
        .records
        .into_iter()
        .map(|record| {
            let source = map_storage_source(record.source);
            ClipboardRecord::from_storage(record, source)
        })
        .collect();
    Ok(ClipboardHistoryPage {
        records,
        has_more: page.has_more,
        next_anchor,
    })
}

pub(crate) async fn load_record_by_event_id(
    state: &WorkspaceState,
    event_id: EventId,
) -> CoreResult<nooboard_storage::HistoryRecord> {
    state
        .storage_runtime()
        .get_event_by_id(event_id.as_uuid())
        .await?
        .ok_or(CoreError::EventNotFound {
            event_id: event_id.to_string(),
        })
}

pub(crate) fn map_storage_source(
    source: nooboard_storage::HistoryRecordSource,
) -> ClipboardRecordSource {
    match source {
        nooboard_storage::HistoryRecordSource::LocalCapture => ClipboardRecordSource::LocalCapture,
        nooboard_storage::HistoryRecordSource::RemoteSync => ClipboardRecordSource::RemoteSync,
        nooboard_storage::HistoryRecordSource::UserSubmit => ClipboardRecordSource::UserSubmit,
    }
}

pub(crate) fn map_clipboard_source(
    source: ClipboardRecordSource,
) -> nooboard_storage::HistoryRecordSource {
    match source {
        ClipboardRecordSource::LocalCapture => nooboard_storage::HistoryRecordSource::LocalCapture,
        ClipboardRecordSource::RemoteSync => nooboard_storage::HistoryRecordSource::RemoteSync,
        ClipboardRecordSource::UserSubmit => nooboard_storage::HistoryRecordSource::UserSubmit,
    }
}

#[cfg(test)]
mod tests {
    use super::{map_clipboard_source, map_storage_source};
    use crate::ClipboardRecordSource;

    #[test]
    fn maps_storage_sources_both_directions() {
        assert_eq!(
            map_storage_source(nooboard_storage::HistoryRecordSource::LocalCapture),
            ClipboardRecordSource::LocalCapture
        );
        assert_eq!(
            map_storage_source(nooboard_storage::HistoryRecordSource::RemoteSync),
            ClipboardRecordSource::RemoteSync
        );
        assert_eq!(
            map_storage_source(nooboard_storage::HistoryRecordSource::UserSubmit),
            ClipboardRecordSource::UserSubmit
        );

        assert_eq!(
            map_clipboard_source(ClipboardRecordSource::LocalCapture),
            nooboard_storage::HistoryRecordSource::LocalCapture
        );
        assert_eq!(
            map_clipboard_source(ClipboardRecordSource::RemoteSync),
            nooboard_storage::HistoryRecordSource::RemoteSync
        );
        assert_eq!(
            map_clipboard_source(ClipboardRecordSource::UserSubmit),
            nooboard_storage::HistoryRecordSource::UserSubmit
        );
    }
}
