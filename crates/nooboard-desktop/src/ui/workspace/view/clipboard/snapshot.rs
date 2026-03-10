use nooboard_app::{ClipboardRecord, ClipboardRecordSource, EventId, NoobId};

use crate::state::live_app::LiveAppStore;
use crate::ui::workspace::view::shared::clock_label_from_millis;

use super::page_state::{
    ClipboardBroadcastScope, ClipboardDetailTab, ClipboardHistoryLoadState, ClipboardPageState,
    ClipboardSelection,
};

#[derive(Clone)]
pub(super) struct ClipboardSnapshot {
    pub latest_record: Option<ClipboardRecord>,
    pub selected_record: Option<ClipboardRecord>,
    pub latest_selected: bool,
    pub history_rows: Vec<ClipboardHistoryRowSnapshot>,
    pub target_rows: Vec<ClipboardTargetSnapshot>,
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
pub(super) struct ClipboardHistoryRowSnapshot {
    pub record: ClipboardRecord,
    pub selected: bool,
}

#[derive(Clone)]
pub(super) struct ClipboardTargetSnapshot {
    pub noob_id: NoobId,
    pub device_id: String,
    pub selected: bool,
    pub interactive: bool,
}

pub(super) fn build_clipboard_snapshot(
    store: &LiveAppStore,
    page_state: &ClipboardPageState,
    cx: &gpui::App,
) -> ClipboardSnapshot {
    let latest_record = store.latest_committed_record().cloned();
    let latest_event_id = store.app_state().clipboard.latest_committed_event_id;
    let selected_record = selected_record(page_state, latest_record.as_ref(), latest_event_id);
    let latest_selected = matches!(page_state.selection, ClipboardSelection::LatestCommitted);
    let history_rows = page_state
        .history_records
        .iter()
        .filter(|record| Some(record.event_id) != latest_event_id)
        .cloned()
        .map(|record| ClipboardHistoryRowSnapshot {
            selected: page_state
                .selection
                .matches(record.event_id, latest_event_id),
            record,
        })
        .collect::<Vec<_>>();
    let target_rows = store
        .app_state()
        .peers
        .connected
        .iter()
        .map(|peer| ClipboardTargetSnapshot {
            noob_id: peer.noob_id.clone(),
            device_id: peer.device_id.clone(),
            selected: page_state.selected_target_noob_ids.contains(&peer.noob_id),
            interactive: page_state.broadcast_scope == ClipboardBroadcastScope::SelectedPeers,
        })
        .collect::<Vec<_>>();
    let connected_target_count = target_rows.len();
    let selected_target_count = match page_state.broadcast_scope {
        ClipboardBroadcastScope::AllConnected => connected_target_count,
        ClipboardBroadcastScope::SelectedPeers => page_state.selected_target_noob_ids.len(),
    };
    let max_text_bytes = store.app_state().settings.storage.max_text_bytes;
    let edit_dirty = page_state.is_edit_dirty(cx);
    let edit_bytes = page_state.edit_bytes(cx);
    let can_enter_edit = selected_record.is_some() || page_state.edit_event_id.is_some();
    let selected_event_id = selected_record.as_ref().map(|record| record.event_id);

    ClipboardSnapshot {
        latest_record,
        selected_record,
        latest_selected,
        history_rows,
        target_rows,
        detail_tab: page_state.detail_tab,
        broadcast_scope: page_state.broadcast_scope,
        connected_target_count,
        selected_target_count,
        loaded_history_count: page_state.history_records.len(),
        max_text_bytes,
        edit_bytes,
        edit_dirty,
        can_submit_edit: page_state.can_submit_edit(max_text_bytes, cx),
        can_enter_edit,
        history_load_state: page_state.history_load_state,
        can_load_more: page_state.can_load_more(),
        feedback: page_state.feedback.clone(),
        submit_in_flight: page_state.submit_in_flight,
        adopt_in_flight: page_state.adopt_in_flight_event_id == selected_event_id,
        rebroadcast_in_flight: page_state.rebroadcast_in_flight_event_id == selected_event_id,
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
    let mut preview = content.replace('\n', " ");
    if preview.chars().count() <= max_chars {
        return preview;
    }
    preview = preview.chars().take(max_chars).collect::<String>();
    format!("{preview}…")
}

pub(super) fn clipboard_record_time_label(record: &ClipboardRecord) -> String {
    clock_label_from_millis(record.created_at_ms)
}

fn selected_record(
    page_state: &ClipboardPageState,
    latest_record: Option<&ClipboardRecord>,
    latest_event_id: Option<EventId>,
) -> Option<ClipboardRecord> {
    match page_state.selection {
        ClipboardSelection::LatestCommitted => latest_record
            .cloned()
            .or_else(|| page_state.history_records.first().cloned()),
        ClipboardSelection::Pinned(event_id) => page_state
            .record_cache
            .get(&event_id)
            .cloned()
            .or_else(|| {
                if latest_event_id == Some(event_id) {
                    latest_record.cloned()
                } else {
                    None
                }
            })
            .or_else(|| {
                page_state
                    .history_records
                    .iter()
                    .find(|record| record.event_id == event_id)
                    .cloned()
            }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_replaces_newlines_and_truncates() {
        assert_eq!(clipboard_record_preview("alpha\nbeta", 32), "alpha beta");
        assert_eq!(clipboard_record_preview("abcdef", 4), "abcd…");
    }
}
