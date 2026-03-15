use std::collections::{HashMap, HashSet};

use nooboard_core::{ClipboardHistoryCursor, ClipboardHistoryPage, ClipboardRecord, EventId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::ui::workspace) enum ClipboardSelection {
    LatestCommitted,
    Pinned(EventId),
}

impl ClipboardSelection {
    pub(in crate::ui::workspace) fn matches(
        self,
        event_id: EventId,
        latest_event_id: Option<EventId>,
    ) -> bool {
        match self {
            Self::LatestCommitted => latest_event_id == Some(event_id),
            Self::Pinned(selected_id) => selected_id == event_id,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::ui::workspace) enum ClipboardHistoryLoadState {
    Idle,
    LoadingInitial,
    LoadingMore,
}

pub(super) struct ClipboardHistoryState {
    selection: ClipboardSelection,
    records: Vec<ClipboardRecord>,
    record_cache: HashMap<EventId, ClipboardRecord>,
    next_cursor: Option<ClipboardHistoryCursor>,
    load_state: ClipboardHistoryLoadState,
    bootstrapped: bool,
}

impl ClipboardHistoryState {
    pub(super) fn new() -> Self {
        Self {
            selection: ClipboardSelection::LatestCommitted,
            records: Vec::new(),
            record_cache: HashMap::new(),
            next_cursor: None,
            load_state: ClipboardHistoryLoadState::Idle,
            bootstrapped: false,
        }
    }

    pub(super) fn selection(&self) -> ClipboardSelection {
        self.selection
    }

    pub(super) fn records(&self) -> &[ClipboardRecord] {
        &self.records
    }

    pub(super) fn load_state(&self) -> ClipboardHistoryLoadState {
        self.load_state
    }

    pub(super) fn bootstrapped(&self) -> bool {
        self.bootstrapped
    }

    pub(super) fn next_cursor(&self) -> Option<ClipboardHistoryCursor> {
        self.next_cursor.clone()
    }

    pub(super) fn can_load_more(&self) -> bool {
        self.next_cursor.is_some() && self.load_state == ClipboardHistoryLoadState::Idle
    }

    pub(super) fn begin_load(&mut self, initial: bool) -> bool {
        if self.load_state != ClipboardHistoryLoadState::Idle {
            return false;
        }

        self.load_state = if initial {
            ClipboardHistoryLoadState::LoadingInitial
        } else {
            ClipboardHistoryLoadState::LoadingMore
        };
        true
    }

    pub(super) fn finish_load(&mut self, page: ClipboardHistoryPage) {
        self.bootstrapped = true;
        self.load_state = ClipboardHistoryLoadState::Idle;
        self.append_history_page(page.records, page.next_cursor);
    }

    pub(super) fn mark_load_failed(&mut self) {
        self.bootstrapped = true;
        self.load_state = ClipboardHistoryLoadState::Idle;
    }

    pub(super) fn select_latest(&mut self) {
        self.selection = ClipboardSelection::LatestCommitted;
    }

    pub(super) fn select_history(&mut self, event_id: EventId) {
        self.selection = ClipboardSelection::Pinned(event_id);
    }

    pub(super) fn has_cached_record(&self, event_id: EventId) -> bool {
        self.record_cache.contains_key(&event_id)
            || self
                .records
                .iter()
                .any(|record| record.event_id == event_id)
    }

    pub(super) fn selected_record(
        &self,
        latest_record: Option<&ClipboardRecord>,
    ) -> Option<ClipboardRecord> {
        let latest_event_id = latest_record.map(|record| record.event_id);
        match self.selection {
            ClipboardSelection::LatestCommitted => latest_record
                .cloned()
                .or_else(|| self.records.first().cloned()),
            ClipboardSelection::Pinned(event_id) => self
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
                    self.records
                        .iter()
                        .find(|record| record.event_id == event_id)
                        .cloned()
                }),
        }
    }

    pub(super) fn cache_record(&mut self, record: ClipboardRecord) {
        self.record_cache.insert(record.event_id, record);
    }

    pub(super) fn promote_record(&mut self, record: ClipboardRecord) {
        self.cache_record(record.clone());
        promote_history_record(&mut self.records, record);
    }

    fn append_history_page(
        &mut self,
        records: Vec<ClipboardRecord>,
        next_cursor: Option<ClipboardHistoryCursor>,
    ) {
        let mut known = self
            .records
            .iter()
            .map(|record| record.event_id)
            .collect::<HashSet<_>>();

        for record in records {
            self.cache_record(record.clone());
            append_unique_history_record(&mut self.records, &mut known, record);
        }
        self.next_cursor = next_cursor;
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
    use nooboard_core::{ClipboardRecordSource, NoobId};

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
