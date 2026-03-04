use std::collections::{BTreeSet, VecDeque};

use super::*;

pub(crate) struct ClipboardPageState {
    history_items: Vec<ClipboardTextItem>,
    remaining_history_pages: VecDeque<ClipboardHistoryPage>,
    selected_target_node_ids: BTreeSet<String>,
    selection: ClipboardSelection,
    history_load_state: ClipboardHistoryLoadState,
    action_feedback: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ClipboardSelection {
    LatestLive,
    History { event_id: Uuid },
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ClipboardHistoryLoadState {
    Idle,
    LoadingMore,
}

impl ClipboardPageState {
    pub(crate) fn new(store: &ClipboardStore) -> Self {
        let mut pages = store.history_pages.iter();
        let history_items = pages
            .next()
            .map(|page| page.items.clone())
            .unwrap_or_default();

        Self {
            history_items,
            remaining_history_pages: pages.cloned().collect(),
            selected_target_node_ids: store
                .default_selected_target_node_ids
                .iter()
                .cloned()
                .collect(),
            selection: ClipboardSelection::LatestLive,
            history_load_state: ClipboardHistoryLoadState::Idle,
            action_feedback: None,
        }
    }

    pub(super) fn active_item(&self, store: &ClipboardStore) -> ClipboardTextItem {
        match self.selection {
            ClipboardSelection::LatestLive => store.latest_live_item().clone(),
            ClipboardSelection::History { event_id } => self
                .history_items
                .iter()
                .find(|item| item.event_id == event_id)
                .cloned()
                .unwrap_or_else(|| store.latest_live_item().clone()),
        }
    }

    pub(super) fn history_items(&self) -> &[ClipboardTextItem] {
        &self.history_items
    }

    pub(super) fn selected_target_count(&self) -> usize {
        self.selected_target_node_ids.len()
    }

    pub(super) fn action_feedback(&self) -> Option<&str> {
        self.action_feedback.as_deref()
    }

    pub(super) fn is_history_selected(&self, event_id: Uuid) -> bool {
        self.selection == ClipboardSelection::History { event_id }
    }

    pub(super) fn target_is_selected(&self, node_id: &str) -> bool {
        self.selected_target_node_ids.contains(node_id)
    }

    pub(super) fn can_load_more(&self) -> bool {
        !self.remaining_history_pages.is_empty()
    }

    pub(super) fn load_more_label(&self) -> &'static str {
        match self.history_load_state {
            ClipboardHistoryLoadState::Idle => "Load More",
            ClipboardHistoryLoadState::LoadingMore => "Loading...",
        }
    }
}

impl WorkspaceView {
    pub(super) fn set_clipboard_feedback(&mut self, message: impl Into<String>) {
        self.clipboard_page.action_feedback = Some(message.into());
    }

    pub(super) fn toggle_clipboard_target(&mut self, node_id: &str, cx: &mut Context<Self>) {
        if self
            .clipboard_page
            .selected_target_node_ids
            .contains(node_id)
        {
            self.clipboard_page.selected_target_node_ids.remove(node_id);
        } else {
            self.clipboard_page
                .selected_target_node_ids
                .insert(node_id.to_string());
        }

        self.set_clipboard_feedback(format!(
            "{} target{} selected.",
            self.clipboard_page.selected_target_count(),
            if self.clipboard_page.selected_target_count() == 1 {
                ""
            } else {
                "s"
            }
        ));
        cx.notify();
    }

    pub(super) fn toggle_clipboard_history_selection(
        &mut self,
        event_id: Uuid,
        cx: &mut Context<Self>,
    ) {
        if self.clipboard_page.is_history_selected(event_id) {
            self.clipboard_page.selection = ClipboardSelection::LatestLive;
            self.set_clipboard_feedback("Showing latest live item.");
        } else {
            self.clipboard_page.selection = ClipboardSelection::History { event_id };
            self.set_clipboard_feedback(format!(
                "History {} active.",
                self.clipboard_short_event_id(event_id)
            ));
        }

        cx.notify();
    }

    pub(super) fn load_more_clipboard_history(&mut self, cx: &mut Context<Self>) {
        if let Some(page) = self.clipboard_page.remaining_history_pages.pop_front() {
            self.clipboard_page.history_load_state = ClipboardHistoryLoadState::LoadingMore;
            let added = page.items.len();
            self.clipboard_page.history_items.extend(page.items);
            self.clipboard_page.history_load_state = ClipboardHistoryLoadState::Idle;
            self.set_clipboard_feedback(format!(
                "{} more record{} loaded.",
                added,
                if added == 1 { "" } else { "s" }
            ));
            cx.notify();
        }
    }

    pub(super) fn store_remote_clipboard_item(
        &mut self,
        item: ClipboardTextItem,
        cx: &mut Context<Self>,
    ) {
        if !item.can_store() {
            return;
        }

        if self
            .clipboard_page
            .history_items
            .iter()
            .all(|history_item| history_item.event_id != item.event_id)
        {
            let stored_item = ClipboardTextItem {
                residency: ClipboardTextResidency::History,
                ..item.clone()
            };
            self.clipboard_page.history_items.insert(0, stored_item);
        }

        self.clipboard_page.selection = ClipboardSelection::History {
            event_id: item.event_id,
        };
        self.set_clipboard_feedback("Stored and selected.");
        cx.notify();
    }

    pub(super) fn delete_history_clipboard_item(
        &mut self,
        event_id: Uuid,
        cx: &mut Context<Self>,
    ) {
        let before = self.clipboard_page.history_items.len();
        self.clipboard_page
            .history_items
            .retain(|item| item.event_id != event_id);

        if self.clipboard_page.history_items.len() != before {
            self.clipboard_page.selection = ClipboardSelection::LatestLive;
            self.set_clipboard_feedback(format!(
                "Deleted {}.",
                self.clipboard_short_event_id(event_id)
            ));
            cx.notify();
        }
    }
}
