use std::collections::{HashMap, VecDeque};

use gpui::{UniformListScrollHandle, point, px};
use nooboard_core::{
    ClipboardHistoryAnchor, ClipboardHistoryDirection, ClipboardHistoryPage, ClipboardRecord,
    EventId,
};

const MAX_HISTORY_PAGES: usize = 6;
const DETAIL_CACHE_LIMIT: usize = 16;
const HISTORY_PAGE_SIZE: usize = 24;

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
    LoadingOlder,
    LoadingNewer,
}

#[derive(Clone, Debug)]
struct LoadedHistoryPage {
    records: Vec<ClipboardRecord>,
}

pub(super) struct ClipboardHistoryState {
    selection: ClipboardSelection,
    pages: VecDeque<LoadedHistoryPage>,
    detail_cache: HashMap<EventId, ClipboardRecord>,
    detail_cache_order: VecDeque<EventId>,
    pending_new_records: VecDeque<ClipboardRecord>,
    older_anchor: Option<ClipboardHistoryAnchor>,
    newer_anchor: Option<ClipboardHistoryAnchor>,
    has_unloaded_older: bool,
    has_unloaded_newer: bool,
    load_state: ClipboardHistoryLoadState,
    bootstrapped: bool,
    scroll_handle: UniformListScrollHandle,
}

impl ClipboardHistoryState {
    pub(super) fn new() -> Self {
        Self {
            selection: ClipboardSelection::LatestCommitted,
            pages: VecDeque::new(),
            detail_cache: HashMap::new(),
            detail_cache_order: VecDeque::new(),
            pending_new_records: VecDeque::new(),
            older_anchor: None,
            newer_anchor: None,
            has_unloaded_older: false,
            has_unloaded_newer: false,
            load_state: ClipboardHistoryLoadState::Idle,
            bootstrapped: false,
            scroll_handle: UniformListScrollHandle::new(),
        }
    }

    pub(super) fn selection(&self) -> ClipboardSelection {
        self.selection
    }

    pub(super) fn records(&self) -> Vec<ClipboardRecord> {
        self.pages
            .iter()
            .flat_map(|page| page.records.iter().cloned())
            .collect()
    }

    pub(super) fn load_state(&self) -> ClipboardHistoryLoadState {
        self.load_state
    }

    pub(super) fn bootstrapped(&self) -> bool {
        self.bootstrapped
    }

    pub(super) fn older_anchor(&self) -> Option<ClipboardHistoryAnchor> {
        self.older_anchor.clone()
    }

    pub(super) fn newer_anchor(&self) -> Option<ClipboardHistoryAnchor> {
        self.newer_anchor.clone()
    }

    pub(super) fn has_newer_gap(&self) -> bool {
        self.has_unloaded_newer && self.newer_anchor.is_some()
    }

    pub(super) fn has_older_gap(&self) -> bool {
        self.has_unloaded_older && self.older_anchor.is_some()
    }

    pub(super) fn can_load_older(&self) -> bool {
        self.has_older_gap() && self.load_state == ClipboardHistoryLoadState::Idle
    }

    pub(super) fn can_load_newer(&self) -> bool {
        self.has_newer_gap() && self.load_state == ClipboardHistoryLoadState::Idle
    }

    pub(super) fn scroll_handle(&self) -> UniformListScrollHandle {
        self.scroll_handle.clone()
    }

    pub(super) fn pending_new_count(&self) -> usize {
        self.pending_new_records.len()
    }

    pub(super) fn should_defer_latest_record(&self) -> bool {
        self.bootstrapped && !self.is_near_top()
    }

    pub(super) fn accept_latest_record(&mut self, record: ClipboardRecord) {
        if self.contains_record(record.event_id)
            || self
                .pending_new_records
                .iter()
                .any(|existing| existing.event_id == record.event_id)
        {
            return;
        }

        if self.should_defer_latest_record() {
            self.queue_pending_new(record);
        } else {
            self.promote_record(record);
        }
    }

    pub(super) fn reveal_pending_new(&mut self) {
        while let Some(record) = self.pending_new_records.pop_back() {
            self.promote_record(record);
        }
        self.scroll_handle
            .0
            .borrow()
            .base_handle
            .set_offset(point(px(0.0), px(0.0)));
    }

    pub(super) fn auto_reveal_pending_if_near_top(&mut self) -> bool {
        if self.pending_new_records.is_empty() || !self.is_near_top() {
            return false;
        }

        self.reveal_pending_new();
        true
    }

    pub(super) fn begin_load(
        &mut self,
        direction: ClipboardHistoryDirection,
        initial: bool,
    ) -> bool {
        if self.load_state != ClipboardHistoryLoadState::Idle {
            return false;
        }

        self.load_state = if initial {
            ClipboardHistoryLoadState::LoadingInitial
        } else {
            match direction {
                ClipboardHistoryDirection::Older => ClipboardHistoryLoadState::LoadingOlder,
                ClipboardHistoryDirection::Newer => ClipboardHistoryLoadState::LoadingNewer,
            }
        };
        true
    }

    pub(super) fn finish_load(
        &mut self,
        direction: ClipboardHistoryDirection,
        page: ClipboardHistoryPage,
    ) {
        self.bootstrapped = true;
        self.load_state = ClipboardHistoryLoadState::Idle;
        match direction {
            ClipboardHistoryDirection::Older => self.append_older_page(page),
            ClipboardHistoryDirection::Newer => self.prepend_newer_page(page),
        }
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
        self.detail_cache.contains_key(&event_id)
            || self
                .pages
                .iter()
                .flat_map(|page| page.records.iter())
                .any(|record| record.event_id == event_id)
    }

    pub(super) fn selected_record(
        &self,
        latest_record: Option<&ClipboardRecord>,
    ) -> Option<ClipboardRecord> {
        let latest_event_id = latest_record.map(|record| record.event_id);
        match self.selection {
            ClipboardSelection::LatestCommitted => latest_record.cloned().or_else(|| {
                self.pages
                    .iter()
                    .flat_map(|page| page.records.iter())
                    .next()
                    .cloned()
            }),
            ClipboardSelection::Pinned(event_id) => self
                .detail_cache
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
                    self.pages
                        .iter()
                        .flat_map(|page| page.records.iter())
                        .find(|record| record.event_id == event_id)
                        .cloned()
                }),
        }
    }

    pub(super) fn cache_record(&mut self, record: ClipboardRecord) {
        self.insert_detail_cache(record);
    }

    pub(super) fn promote_record(&mut self, record: ClipboardRecord) {
        self.remove_record(record.event_id);
        if let Some(page) = self.pages.front_mut() {
            page.records.insert(0, record);
        } else {
            self.pages.push_front(LoadedHistoryPage {
                records: vec![record],
            });
        }
        self.rebalance_front_pages();
    }

    fn append_older_page(&mut self, page: ClipboardHistoryPage) {
        let records = page
            .records
            .into_iter()
            .filter(|record| !self.contains_record(record.event_id))
            .collect::<Vec<_>>();

        self.has_unloaded_older = page.has_more;
        self.older_anchor = if page.has_more {
            page.next_anchor
        } else {
            None
        };

        if records.is_empty() {
            return;
        }

        self.pages.push_back(LoadedHistoryPage { records });
        self.discard_loaded_pending_records();
        while self.pages.len() > MAX_HISTORY_PAGES {
            if let Some(evicted) = self.pages.pop_front() {
                self.retain_selected_record(&evicted.records);
                self.has_unloaded_newer = true;
                self.newer_anchor = self.current_newest_anchor();
            }
        }
        if !self.has_unloaded_newer {
            self.newer_anchor = None;
        }
    }

    fn prepend_newer_page(&mut self, page: ClipboardHistoryPage) {
        let records = page
            .records
            .into_iter()
            .filter(|record| !self.contains_record(record.event_id))
            .collect::<Vec<_>>();

        self.has_unloaded_newer = page.has_more;
        self.newer_anchor = if page.has_more {
            page.next_anchor
        } else {
            None
        };

        if records.is_empty() {
            return;
        }

        self.pages.push_front(LoadedHistoryPage { records });
        self.discard_loaded_pending_records();
        while self.pages.len() > MAX_HISTORY_PAGES {
            if let Some(evicted) = self.pages.pop_back() {
                self.retain_selected_record(&evicted.records);
                self.has_unloaded_older = true;
                self.older_anchor = self.current_oldest_anchor();
            }
        }
        if !self.has_unloaded_older {
            self.older_anchor = None;
        }
    }

    fn contains_record(&self, event_id: EventId) -> bool {
        self.pages
            .iter()
            .flat_map(|page| page.records.iter())
            .any(|record| record.event_id == event_id)
    }

    fn remove_record(&mut self, event_id: EventId) {
        for page in &mut self.pages {
            page.records.retain(|record| record.event_id != event_id);
        }
        self.pages.retain(|page| !page.records.is_empty());
    }

    fn current_newest_anchor(&self) -> Option<ClipboardHistoryAnchor> {
        self.pages
            .front()
            .and_then(|page| page.records.first())
            .map(record_anchor)
    }

    fn current_oldest_anchor(&self) -> Option<ClipboardHistoryAnchor> {
        self.pages
            .back()
            .and_then(|page| page.records.last())
            .map(record_anchor)
    }

    fn retain_selected_record(&mut self, records: &[ClipboardRecord]) {
        let ClipboardSelection::Pinned(selected_id) = self.selection else {
            return;
        };
        if let Some(record) = records.iter().find(|record| record.event_id == selected_id) {
            self.insert_detail_cache(record.clone());
        }
    }

    fn insert_detail_cache(&mut self, record: ClipboardRecord) {
        let event_id = record.event_id;
        self.detail_cache.insert(event_id, record);
        self.touch_detail_cache(event_id);
        self.prune_detail_cache();
    }

    fn queue_pending_new(&mut self, record: ClipboardRecord) {
        self.pending_new_records
            .retain(|existing| existing.event_id != record.event_id);
        self.pending_new_records.push_front(record);
    }

    fn discard_loaded_pending_records(&mut self) {
        let loaded_ids = self
            .pages
            .iter()
            .flat_map(|page| page.records.iter().map(|record| record.event_id))
            .collect::<Vec<_>>();
        self.pending_new_records
            .retain(|record| !loaded_ids.contains(&record.event_id));
    }

    fn touch_detail_cache(&mut self, event_id: EventId) {
        self.detail_cache_order
            .retain(|existing| *existing != event_id);
        self.detail_cache_order.push_back(event_id);
    }

    fn prune_detail_cache(&mut self) {
        let selected_id = match self.selection {
            ClipboardSelection::Pinned(event_id) => Some(event_id),
            ClipboardSelection::LatestCommitted => None,
        };

        while self.detail_cache.len() > DETAIL_CACHE_LIMIT {
            let Some(candidate) = self.detail_cache_order.pop_front() else {
                break;
            };
            if Some(candidate) == selected_id {
                self.detail_cache_order.push_back(candidate);
                break;
            }
            self.detail_cache.remove(&candidate);
        }
    }

    fn is_near_top(&self) -> bool {
        self.scroll_handle
            .0
            .borrow()
            .base_handle
            .logical_scroll_top()
            .0
            <= 1
    }

    fn rebalance_front_pages(&mut self) {
        let mut index = 0;
        let mut carry: Option<Vec<ClipboardRecord>> = None;

        loop {
            if let Some(carry_records) = carry.take() {
                if let Some(page) = self.pages.get_mut(index) {
                    page.records.splice(0..0, carry_records);
                } else {
                    self.pages.push_back(LoadedHistoryPage {
                        records: carry_records,
                    });
                }
            }

            let Some(page) = self.pages.get_mut(index) else {
                break;
            };
            if page.records.len() <= HISTORY_PAGE_SIZE {
                index += 1;
                if index >= self.pages.len() {
                    break;
                }
                continue;
            }

            carry = Some(page.records.split_off(HISTORY_PAGE_SIZE));
            index += 1;
        }

        while self.pages.len() > MAX_HISTORY_PAGES {
            if let Some(evicted) = self.pages.pop_back() {
                self.retain_selected_record(&evicted.records);
                self.has_unloaded_older = true;
                self.older_anchor = self.current_oldest_anchor();
            }
        }
    }
}

fn record_anchor(record: &ClipboardRecord) -> ClipboardHistoryAnchor {
    ClipboardHistoryAnchor {
        created_at_ms: record.created_at_ms,
        event_id: record.event_id,
    }
}

#[cfg(test)]
mod tests {
    use nooboard_core::{ClipboardRecordSource, NoobId};

    use super::*;

    fn record(event_id: EventId, content: &str, created_at_ms: i64) -> ClipboardRecord {
        ClipboardRecord {
            event_id,
            source: ClipboardRecordSource::UserSubmit,
            origin_noob_id: NoobId::new("peer-a"),
            origin_device_id: "peer-a-device".to_string(),
            created_at_ms,
            applied_at_ms: created_at_ms,
            content: content.to_string(),
        }
    }

    #[test]
    fn promote_record_moves_existing_record_to_front() {
        let first_id = EventId::new();
        let second_id = EventId::new();
        let mut state = ClipboardHistoryState::new();
        state.pages.push_back(LoadedHistoryPage {
            records: vec![
                record(first_id, "first", 10),
                record(second_id, "second", 20),
            ],
        });

        state.promote_record(record(second_id, "second", 20));

        let records = state.records();
        assert_eq!(records[0].event_id, second_id);
        assert_eq!(records[1].event_id, first_id);
    }

    #[test]
    fn loading_older_pages_sets_newer_gap_when_window_evicted() {
        let mut state = ClipboardHistoryState::new();
        for page_ix in 0..=MAX_HISTORY_PAGES {
            let event_id = EventId::new();
            state.finish_load(
                ClipboardHistoryDirection::Older,
                ClipboardHistoryPage {
                    records: vec![record(event_id, "row", page_ix as i64)],
                    has_more: page_ix < MAX_HISTORY_PAGES,
                    next_anchor: Some(ClipboardHistoryAnchor {
                        created_at_ms: page_ix as i64,
                        event_id,
                    }),
                },
            );
        }

        assert!(state.has_newer_gap());
        assert_eq!(state.pages.len(), MAX_HISTORY_PAGES);
    }
}
