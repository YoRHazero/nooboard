use nooboard_core::{ClipboardRecord, ClipboardRecordSource, SessionId};
use time::{OffsetDateTime, UtcOffset};

use crate::workspace::view_state::ClipboardWorkspaceViewState;

use super::state::{
    ClipboardBroadcastScope, ClipboardDetailTab, ClipboardHistoryLoadState, ClipboardPageState,
};

#[derive(Clone)]
pub(super) struct ClipboardPageViewState {
    pub page_ready: bool,
    pub latest_record: Option<ClipboardRecord>,
    pub selected_record: Option<ClipboardRecord>,
    pub latest_selected: bool,
    pub history_rows: Vec<ClipboardHistoryRowViewState>,
    pub target_rows: Vec<ClipboardTargetViewState>,
    pub detail_tab: ClipboardDetailTab,
    pub broadcast_scope: ClipboardBroadcastScope,
    pub connected_target_count: usize,
    pub selected_target_count: usize,
    pub loaded_history_count: usize,
    pub max_text_bytes: usize,
    pub edit_bytes: usize,
    pub edit_dirty: bool,
    pub can_submit_edit: bool,
    pub can_enter_edit: bool,
    pub history_load_state: ClipboardHistoryLoadState,
    pub can_load_more: bool,
    pub feedback: Option<String>,
    pub submit_in_flight: bool,
    pub adopt_in_flight: bool,
    pub rebroadcast_in_flight: bool,
}

#[derive(Clone)]
pub(super) struct ClipboardHistoryRowViewState {
    pub record: ClipboardRecord,
    pub selected: bool,
}

#[derive(Clone)]
pub(super) struct ClipboardTargetViewState {
    pub id: SessionId,
    pub device_id: String,
    pub secondary_label: String,
    pub selected: bool,
    pub interactive: bool,
}

pub(super) fn build_clipboard_page_view_state(
    page: Option<&ClipboardWorkspaceViewState>,
    state: &ClipboardPageState,
    cx: &gpui::Context<super::super::WorkspaceView>,
) -> ClipboardPageViewState {
    let Some(page) = page else {
        return ClipboardPageViewState {
            page_ready: false,
            latest_record: None,
            selected_record: None,
            latest_selected: false,
            history_rows: Vec::new(),
            target_rows: Vec::new(),
            detail_tab: ClipboardDetailTab::Read,
            broadcast_scope: ClipboardBroadcastScope::AllConnected,
            connected_target_count: 0,
            selected_target_count: 0,
            loaded_history_count: 0,
            max_text_bytes: 0,
            edit_bytes: 0,
            edit_dirty: false,
            can_submit_edit: false,
            can_enter_edit: false,
            history_load_state: ClipboardHistoryLoadState::Idle,
            can_load_more: false,
            feedback: None,
            submit_in_flight: false,
            adopt_in_flight: false,
            rebroadcast_in_flight: false,
        };
    };

    let latest_record = page.latest_record.clone();
    let latest_event_id = latest_record.as_ref().map(|record| record.event_id);
    let selected_record = state.selected_record(latest_record.as_ref());
    let selected_event_id = selected_record.as_ref().map(|record| record.event_id);
    let target_rows = page
        .session_targets
        .iter()
        .map(|target| ClipboardTargetViewState {
            id: target.id,
            device_id: target.device_id.clone(),
            secondary_label: format!("{} · {}", target.mode_label, target.remote_addr_label),
            selected: state.selected_session_ids().contains(&target.id),
            interactive: state.broadcast_scope() == ClipboardBroadcastScope::SelectedSessions,
        })
        .collect::<Vec<_>>();
    let connected_target_count = target_rows.len();
    let selected_target_count = match state.broadcast_scope() {
        ClipboardBroadcastScope::AllConnected => connected_target_count,
        ClipboardBroadcastScope::SelectedSessions => state.selected_session_ids().len(),
    };

    ClipboardPageViewState {
        page_ready: true,
        latest_record,
        selected_record,
        latest_selected: matches!(
            state.selection(),
            super::state::ClipboardSelection::LatestCommitted
        ),
        history_rows: state
            .history_records()
            .iter()
            .filter(|record| Some(record.event_id) != latest_event_id)
            .cloned()
            .map(|record| ClipboardHistoryRowViewState {
                selected: state.selection().matches(record.event_id, latest_event_id),
                record,
            })
            .collect(),
        target_rows,
        detail_tab: state.detail_tab(),
        broadcast_scope: state.broadcast_scope(),
        connected_target_count,
        selected_target_count,
        loaded_history_count: state.history_records().len(),
        max_text_bytes: page.max_text_bytes,
        edit_bytes: state.edit_bytes(cx),
        edit_dirty: state.is_edit_dirty(cx),
        can_submit_edit: state.can_submit_edit(page.max_text_bytes, cx),
        can_enter_edit: selected_event_id.is_some(),
        history_load_state: state.history_load_state(),
        can_load_more: state.can_load_more(),
        feedback: state.feedback().cloned(),
        submit_in_flight: state.submit_in_flight(),
        adopt_in_flight: state.adopt_in_flight_event_id() == selected_event_id,
        rebroadcast_in_flight: state.rebroadcast_in_flight_event_id() == selected_event_id,
    }
}

pub(super) fn clipboard_source_label(source: ClipboardRecordSource) -> &'static str {
    match source {
        ClipboardRecordSource::LocalCapture => "Local Capture",
        ClipboardRecordSource::RemoteSync => "Remote Sync",
        ClipboardRecordSource::UserSubmit => "User Submit",
    }
}

pub(super) fn clipboard_record_preview(content: &str, max_chars: usize) -> String {
    let preview = content.replace('\n', " ");
    let mut chars = preview.chars();
    let compact = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        format!("{compact}…")
    } else {
        compact
    }
}

pub(super) fn clipboard_record_time_label(record: &ClipboardRecord) -> String {
    let offset = UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC);
    let nanos = i128::from(record.created_at_ms) * 1_000_000;
    let datetime = OffsetDateTime::from_unix_timestamp_nanos(nanos)
        .unwrap_or(OffsetDateTime::UNIX_EPOCH)
        .to_offset(offset);

    format!(
        "{:02}:{:02}:{:02}",
        datetime.hour(),
        datetime.minute(),
        datetime.second()
    )
}

pub(super) fn clipboard_short_event_id(record: &ClipboardRecord) -> String {
    record
        .event_id
        .as_uuid()
        .simple()
        .to_string()
        .chars()
        .take(8)
        .collect()
}

#[cfg(test)]
mod tests {
    use nooboard_core::{ClipboardRecordSource, EventId, NoobId};

    use super::*;

    #[test]
    fn preview_replaces_newlines_and_truncates() {
        assert_eq!(clipboard_record_preview("alpha\nbeta", 32), "alpha beta");
        assert_eq!(clipboard_record_preview("abcdef", 4), "abcd…");
    }

    #[test]
    fn source_label_maps_all_core_variants() {
        assert_eq!(
            clipboard_source_label(ClipboardRecordSource::LocalCapture),
            "Local Capture"
        );
        assert_eq!(
            clipboard_source_label(ClipboardRecordSource::RemoteSync),
            "Remote Sync"
        );
        assert_eq!(
            clipboard_source_label(ClipboardRecordSource::UserSubmit),
            "User Submit"
        );
    }

    #[test]
    fn short_event_id_is_trimmed() {
        let record = ClipboardRecord {
            event_id: EventId::new(),
            source: ClipboardRecordSource::UserSubmit,
            origin_noob_id: NoobId::new("local"),
            origin_device_id: "desk-01".to_string(),
            created_at_ms: 0,
            applied_at_ms: 0,
            content: "hello".to_string(),
        };

        assert_eq!(clipboard_short_event_id(&record).len(), 8);
    }
}
