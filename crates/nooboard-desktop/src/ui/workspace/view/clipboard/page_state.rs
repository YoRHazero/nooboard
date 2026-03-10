use std::collections::{BTreeSet, HashMap, HashSet};

use gpui::{AppContext, Context, Entity, Window};
use gpui_component::input::InputState;
use nooboard_app::{ClipboardHistoryCursor, ClipboardRecord, EventId, NoobId};

use crate::state::WorkspaceRoute;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ClipboardSelection {
    LatestCommitted,
    Pinned(EventId),
}

impl ClipboardSelection {
    pub(super) fn matches(self, event_id: EventId, latest_event_id: Option<EventId>) -> bool {
        match self {
            Self::LatestCommitted => latest_event_id == Some(event_id),
            Self::Pinned(selected_id) => selected_id == event_id,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ClipboardDetailTab {
    Read,
    Edit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ClipboardBroadcastScope {
    AllConnected,
    SelectedPeers,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ClipboardHistoryLoadState {
    Idle,
    LoadingInitial,
    LoadingMore,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ClipboardExitIntent {
    SelectLatest,
    SelectHistory(EventId),
    SwitchTab(ClipboardDetailTab),
    NavigateRoute(WorkspaceRoute),
}

pub(crate) struct ClipboardPageState {
    pub(super) selection: ClipboardSelection,
    pub(super) detail_tab: ClipboardDetailTab,
    pub(super) broadcast_scope: ClipboardBroadcastScope,
    pub(super) history_records: Vec<ClipboardRecord>,
    pub(super) record_cache: HashMap<EventId, ClipboardRecord>,
    pub(super) next_cursor: Option<ClipboardHistoryCursor>,
    pub(super) history_load_state: ClipboardHistoryLoadState,
    pub(super) history_bootstrapped: bool,
    pub(super) latest_seen_committed_event_id: Option<EventId>,
    pub(super) selected_target_noob_ids: BTreeSet<NoobId>,
    pub(super) feedback: Option<String>,
    pub(super) read_input: Entity<InputState>,
    pub(super) read_event_id: Option<EventId>,
    pub(super) edit_input: Entity<InputState>,
    pub(super) edit_event_id: Option<EventId>,
    pub(super) edit_base_content: String,
    pub(super) discard_confirm_open: bool,
    pub(super) submit_in_flight: bool,
    pub(super) adopt_in_flight_event_id: Option<EventId>,
    pub(super) rebroadcast_in_flight_event_id: Option<EventId>,
}

impl ClipboardPageState {
    pub(crate) fn new(
        latest_record: Option<ClipboardRecord>,
        latest_committed_event_id: Option<EventId>,
        window: &mut Window,
        cx: &mut Context<super::WorkspaceView>,
    ) -> Self {
        let read_input = cx.new(|cx| {
            InputState::new(window, cx)
                .multi_line(true)
                .rows(14)
                .placeholder("Selected committed clipboard content will appear here.")
        });
        let edit_input = cx.new(|cx| {
            InputState::new(window, cx)
                .multi_line(true)
                .rows(14)
                .placeholder("Edit this committed clipboard record and save it as a new event.")
        });

        let mut history_records = Vec::new();
        let mut record_cache = HashMap::new();
        if let Some(record) = latest_record {
            record_cache.insert(record.event_id, record.clone());
            history_records.push(record);
        }

        Self {
            selection: ClipboardSelection::LatestCommitted,
            detail_tab: ClipboardDetailTab::Read,
            broadcast_scope: ClipboardBroadcastScope::AllConnected,
            history_records,
            record_cache,
            next_cursor: None,
            history_load_state: ClipboardHistoryLoadState::Idle,
            history_bootstrapped: false,
            latest_seen_committed_event_id: latest_committed_event_id,
            selected_target_noob_ids: BTreeSet::new(),
            feedback: None,
            read_input,
            read_event_id: None,
            edit_input,
            edit_event_id: None,
            edit_base_content: String::new(),
            discard_confirm_open: false,
            submit_in_flight: false,
            adopt_in_flight_event_id: None,
            rebroadcast_in_flight_event_id: None,
        }
    }

    pub(super) fn can_load_more(&self) -> bool {
        self.next_cursor.is_some() && self.history_load_state == ClipboardHistoryLoadState::Idle
    }

    pub(crate) fn edit_input(&self) -> Entity<InputState> {
        self.edit_input.clone()
    }

    pub(super) fn sync_read_record(
        &mut self,
        record: Option<&ClipboardRecord>,
        window: &mut Window,
        cx: &mut Context<super::WorkspaceView>,
    ) {
        let next_event_id = record.map(|record| record.event_id);
        if self.read_event_id == next_event_id {
            return;
        }

        let next_content = record
            .map(|record| record.content.clone())
            .unwrap_or_default();
        self.read_event_id = next_event_id;
        self.read_input.update(cx, |input, cx| {
            input.set_value(next_content, window, cx);
        });
    }

    pub(super) fn is_edit_dirty(&self, cx: &gpui::App) -> bool {
        self.edit_event_id.is_some() && self.edit_text(cx) != self.edit_base_content
    }

    pub(super) fn edit_text(&self, cx: &gpui::App) -> String {
        self.edit_input.read(cx).value().to_string()
    }

    pub(super) fn edit_bytes(&self, cx: &gpui::App) -> usize {
        self.edit_text(cx).len()
    }

    pub(super) fn can_submit_edit(&self, max_text_bytes: usize, cx: &gpui::App) -> bool {
        let edit_bytes = self.edit_bytes(cx);
        self.edit_event_id.is_some()
            && !self.submit_in_flight
            && self.edit_text(cx) != self.edit_base_content
            && edit_bytes <= max_text_bytes
    }

    pub(super) fn retain_connected_targets(&mut self, connected: &HashSet<NoobId>) {
        self.selected_target_noob_ids
            .retain(|noob_id| connected.contains(noob_id));
    }

    pub(super) fn cache_record(&mut self, record: ClipboardRecord) {
        self.record_cache.insert(record.event_id, record);
    }

    pub(super) fn promote_record(&mut self, record: ClipboardRecord) {
        self.cache_record(record.clone());
        promote_history_record(&mut self.history_records, record);
    }

    pub(super) fn append_history_page(
        &mut self,
        records: Vec<ClipboardRecord>,
        next_cursor: Option<ClipboardHistoryCursor>,
    ) {
        let mut known = self
            .history_records
            .iter()
            .map(|record| record.event_id)
            .collect::<HashSet<_>>();
        for record in records {
            self.cache_record(record.clone());
            append_unique_history_record(&mut self.history_records, &mut known, record);
        }
        self.next_cursor = next_cursor;
    }

    pub(super) fn begin_edit_session(
        &mut self,
        record: &ClipboardRecord,
        window: &mut Window,
        cx: &mut Context<super::WorkspaceView>,
    ) {
        self.detail_tab = ClipboardDetailTab::Edit;
        self.edit_event_id = Some(record.event_id);
        self.edit_base_content = record.content.clone();
        self.edit_input.update(cx, |input, cx| {
            input.set_value(record.content.clone(), window, cx);
        });
    }

    pub(super) fn clear_edit_session(
        &mut self,
        window: &mut Window,
        cx: &mut Context<super::WorkspaceView>,
    ) {
        self.detail_tab = ClipboardDetailTab::Read;
        self.edit_event_id = None;
        self.edit_base_content.clear();
        self.edit_input.update(cx, |input, cx| {
            input.set_value("", window, cx);
        });
    }
}

fn promote_history_record(history_records: &mut Vec<ClipboardRecord>, record: ClipboardRecord) {
    history_records.retain(|existing| existing.event_id != record.event_id);
    history_records.insert(0, record);
}

fn append_unique_history_record(
    history_records: &mut Vec<ClipboardRecord>,
    known: &mut HashSet<EventId>,
    record: ClipboardRecord,
) {
    if known.insert(record.event_id) {
        history_records.push(record);
    }
}

#[cfg(test)]
mod tests {
    use nooboard_app::{ClipboardRecordSource, EventId, NoobId};

    use super::*;

    fn record(event_id: EventId, content: &str) -> ClipboardRecord {
        ClipboardRecord {
            event_id,
            source: ClipboardRecordSource::UserSubmit,
            origin_noob_id: NoobId::new("peer-a"),
            origin_device_id: "peer-a-device".to_string(),
            created_at_ms: 10,
            applied_at_ms: 10,
            content: content.to_string(),
        }
    }

    #[test]
    fn promote_record_moves_existing_record_to_front() {
        let first_id = EventId::new();
        let second_id = EventId::new();
        let mut history = vec![record(first_id, "first"), record(second_id, "second")];

        promote_history_record(&mut history, record(second_id, "second"));

        assert_eq!(history[0].event_id, second_id);
        assert_eq!(history[1].event_id, first_id);
        assert_eq!(history.len(), 2);
    }

    #[test]
    fn append_history_page_dedupes_existing_records() {
        let first_id = EventId::new();
        let second_id = EventId::new();
        let third_id = EventId::new();
        let mut history = vec![record(first_id, "first"), record(second_id, "second")];
        let mut known = history
            .iter()
            .map(|record| record.event_id)
            .collect::<HashSet<_>>();

        append_unique_history_record(&mut history, &mut known, record(second_id, "second"));
        append_unique_history_record(&mut history, &mut known, record(third_id, "third"));

        assert_eq!(history.len(), 3);
        assert_eq!(history[2].event_id, third_id);
    }
}
