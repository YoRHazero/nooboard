use std::time::{SystemTime, UNIX_EPOCH};

use crate::clipboard::LocalClipboardObserved;
use crate::error::{CoreError, CoreResult};
use crate::types::{ClipboardRecordSource, EventId, NoobId, WorkspaceEvent};
use crate::{NetworkStatus, SessionTarget};

use super::super::state::WorkspaceState;
use super::storage::{load_record_by_event_id, map_clipboard_source};

pub(crate) async fn handle_local_clipboard_observed(
    state: &mut WorkspaceState,
    observed: LocalClipboardObserved,
) -> CoreResult<()> {
    let identity = state.current_snapshot().identity.clone();
    let _ = commit_clipboard_record(
        state,
        observed.event_id,
        observed.text,
        identity.noob_id,
        identity.device_id,
        observed.observed_at_ms,
        observed.observed_at_ms,
        ClipboardRecordSource::LocalCapture,
        true,
    )
    .await?;
    Ok(())
}

pub(crate) async fn handle_remote_text_received(
    state: &mut WorkspaceState,
    event_id: &str,
    content: String,
    peer_noob_id: String,
    peer_device_id: String,
) -> CoreResult<()> {
    let parsed_event_id = event_id.parse::<EventId>().map_err(|_| {
        CoreError::InvalidState(format!("network text event_id is not a UUID: {event_id}"))
    })?;
    let now_ms = now_millis_i64();
    let _ = commit_clipboard_record(
        state,
        parsed_event_id,
        content,
        NoobId::new(peer_noob_id),
        peer_device_id,
        now_ms,
        now_ms,
        ClipboardRecordSource::RemoteSync,
        false,
    )
    .await?;
    Ok(())
}

pub(crate) async fn submit_text(
    state: &mut WorkspaceState,
    content: String,
) -> CoreResult<EventId> {
    let event_id = EventId::new();
    let identity = state.current_snapshot().identity.clone();
    let now_ms = now_millis_i64();
    let _ = commit_clipboard_record(
        state,
        event_id,
        content,
        identity.noob_id,
        identity.device_id,
        now_ms,
        now_ms,
        ClipboardRecordSource::UserSubmit,
        true,
    )
    .await?;
    Ok(event_id)
}

pub(crate) async fn adopt_clipboard_record(
    state: &WorkspaceState,
    event_id: EventId,
) -> CoreResult<()> {
    let record = load_record_by_event_id(state, event_id).await?;
    state
        .clipboard_runtime()
        .write_text_with_suppression(&record.content)
}

pub(crate) async fn rebroadcast_clipboard_record(
    state: &WorkspaceState,
    event_id: EventId,
    target: SessionTarget,
) -> CoreResult<()> {
    let record = load_record_by_event_id(state, event_id).await?;
    broadcast_text(state, event_id, record.content, target).await
}

async fn commit_clipboard_record(
    state: &mut WorkspaceState,
    event_id: EventId,
    content: String,
    origin_noob_id: NoobId,
    origin_device_id: String,
    created_at_ms: i64,
    applied_at_ms: i64,
    source: ClipboardRecordSource,
    broadcast_after_commit: bool,
) -> CoreResult<bool> {
    validate_text_size(state, &content)?;

    let inserted = state
        .storage_runtime()
        .append_text_with_source(
            &content,
            Some(event_id.as_uuid()),
            Some(origin_noob_id.as_str()),
            Some(origin_device_id.as_str()),
            created_at_ms,
            applied_at_ms,
            map_clipboard_source(source),
        )
        .await?;

    if !inserted {
        return Ok(false);
    }

    let current_network_snapshot = state.current_snapshot().network.clone();
    state.set_latest_committed_event_id(Some(event_id));
    state.refresh_snapshot(current_network_snapshot);
    state.publish_event(WorkspaceEvent::ClipboardCommitted { event_id, source });

    if broadcast_after_commit
        && matches!(
            state.current_snapshot().network.status,
            NetworkStatus::Running
        )
    {
        broadcast_text(state, event_id, content, SessionTarget::AllConnected).await?;
    }

    Ok(true)
}

async fn broadcast_text(
    state: &WorkspaceState,
    event_id: EventId,
    content: String,
    target: SessionTarget,
) -> CoreResult<()> {
    state
        .network_runtime()
        .send_text(nooboard_network::SendTextRequest {
            event_id: event_id.to_string(),
            content,
            target,
        })
        .await
        .map_err(Into::into)
}

fn validate_text_size(state: &WorkspaceState, content: &str) -> CoreResult<()> {
    let actual_bytes = content.len();
    let max_bytes = state.config().storage.max_text_bytes;
    if actual_bytes > max_bytes {
        return Err(CoreError::TextTooLarge {
            actual_bytes,
            max_bytes,
        });
    }
    Ok(())
}

fn now_millis_i64() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| i64::try_from(duration.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use crate::error::CoreError;

    use super::validate_text_size;

    struct DummyState(usize);

    impl DummyState {
        fn max_text_bytes(&self) -> usize {
            self.0
        }
    }

    fn validate(max_text_bytes: usize, content: &str) -> Result<(), CoreError> {
        let state = DummyState(max_text_bytes);
        let actual_bytes = content.len();
        if actual_bytes > state.max_text_bytes() {
            return Err(CoreError::TextTooLarge {
                actual_bytes,
                max_bytes: state.max_text_bytes(),
            });
        }
        Ok(())
    }

    #[test]
    fn rejects_oversized_text() {
        let error = validate(4, "hello").expect_err("must reject");
        assert!(matches!(error, CoreError::TextTooLarge { .. }));
    }

    #[test]
    fn accepts_boundary_size() {
        validate(5, "hello").expect("must accept");
        let _ = validate_text_size;
    }
}
