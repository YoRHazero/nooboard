use nooboard_core::{ClipboardRecord, EventId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::ui::workspace) enum ClipboardDetailTab {
    Read,
    Edit,
}

pub(super) struct ClipboardEditorState {
    detail_tab: ClipboardDetailTab,
    edit_event_id: Option<EventId>,
    edit_base_content: String,
    submit_in_flight: bool,
}

impl ClipboardEditorState {
    pub(super) fn new() -> Self {
        Self {
            detail_tab: ClipboardDetailTab::Read,
            edit_event_id: None,
            edit_base_content: String::new(),
            submit_in_flight: false,
        }
    }

    pub(super) fn detail_tab(&self) -> ClipboardDetailTab {
        self.detail_tab
    }

    pub(super) fn submit_in_flight(&self) -> bool {
        self.submit_in_flight
    }

    pub(super) fn begin_session(&mut self, record: &ClipboardRecord) {
        self.detail_tab = ClipboardDetailTab::Edit;
        self.edit_event_id = Some(record.event_id);
        self.edit_base_content = record.content.clone();
    }

    pub(super) fn clear_session(&mut self) {
        self.detail_tab = ClipboardDetailTab::Read;
        self.edit_event_id = None;
        self.edit_base_content.clear();
    }

    pub(super) fn is_dirty(&self, current_text: &str) -> bool {
        self.edit_event_id.is_some() && current_text != self.edit_base_content
    }

    pub(super) fn can_submit(&self, current_text: &str, max_text_bytes: usize) -> bool {
        self.edit_event_id.is_some()
            && !self.submit_in_flight
            && current_text != self.edit_base_content
            && current_text.len() <= max_text_bytes
    }

    pub(super) fn start_submit(&mut self) {
        self.submit_in_flight = true;
    }

    pub(super) fn finish_submit(&mut self) {
        self.submit_in_flight = false;
        self.clear_session();
    }

    pub(super) fn fail_submit(&mut self) {
        self.submit_in_flight = false;
    }
}
