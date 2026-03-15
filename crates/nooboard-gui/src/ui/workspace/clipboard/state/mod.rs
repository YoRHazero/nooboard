mod editor;
mod history;
mod inputs;
mod targets;

use gpui::{Context, Entity, Window};
use gpui_component::input::InputState;
use nooboard_core::{
    ClipboardHistoryCursor, ClipboardHistoryPage, ClipboardRecord, EventId, SessionId,
    SessionTarget,
};

use crate::{ui::workspace::WorkspaceView, workspace::view_state::ClipboardWorkspaceViewState};

pub(in crate::ui::workspace) use editor::ClipboardDetailTab;
pub(in crate::ui::workspace) use history::{ClipboardHistoryLoadState, ClipboardSelection};
pub(in crate::ui::workspace) use targets::ClipboardBroadcastScope;

use editor::ClipboardEditorState;
use history::ClipboardHistoryState;
use inputs::ClipboardInputs;
use targets::ClipboardTargetSelectionState;

pub(in crate::ui::workspace) struct ClipboardPageState {
    history: ClipboardHistoryState,
    editor: ClipboardEditorState,
    targets: ClipboardTargetSelectionState,
    inputs: ClipboardInputs,
    feedback: Option<String>,
}

impl ClipboardPageState {
    pub(in crate::ui::workspace) fn new(
        window: &mut Window,
        cx: &mut Context<WorkspaceView>,
    ) -> Self {
        Self {
            history: ClipboardHistoryState::new(),
            editor: ClipboardEditorState::new(),
            targets: ClipboardTargetSelectionState::new(),
            inputs: ClipboardInputs::new(window, cx),
            feedback: None,
        }
    }

    pub(in crate::ui::workspace) fn edit_input(&self) -> Entity<InputState> {
        self.inputs.edit_input()
    }

    pub(in crate::ui::workspace) fn read_input(&self) -> Entity<InputState> {
        self.inputs.read_input()
    }

    pub(in crate::ui::workspace) fn selection(&self) -> ClipboardSelection {
        self.history.selection()
    }

    pub(in crate::ui::workspace) fn detail_tab(&self) -> ClipboardDetailTab {
        self.editor.detail_tab()
    }

    pub(in crate::ui::workspace) fn broadcast_scope(&self) -> ClipboardBroadcastScope {
        self.targets.broadcast_scope()
    }

    pub(in crate::ui::workspace) fn history_records(&self) -> &[ClipboardRecord] {
        self.history.records()
    }

    pub(in crate::ui::workspace) fn history_load_state(&self) -> ClipboardHistoryLoadState {
        self.history.load_state()
    }

    pub(in crate::ui::workspace) fn history_bootstrapped(&self) -> bool {
        self.history.bootstrapped()
    }

    pub(in crate::ui::workspace) fn next_cursor(&self) -> Option<ClipboardHistoryCursor> {
        self.history.next_cursor()
    }

    pub(in crate::ui::workspace) fn selected_session_ids(
        &self,
    ) -> &std::collections::BTreeSet<SessionId> {
        self.targets.selected_session_ids()
    }

    pub(in crate::ui::workspace) fn feedback(&self) -> Option<&String> {
        self.feedback.as_ref()
    }

    pub(in crate::ui::workspace) fn submit_in_flight(&self) -> bool {
        self.editor.submit_in_flight()
    }

    pub(in crate::ui::workspace) fn adopt_in_flight_event_id(&self) -> Option<EventId> {
        self.targets.adopt_in_flight_event_id()
    }

    pub(in crate::ui::workspace) fn rebroadcast_in_flight_event_id(&self) -> Option<EventId> {
        self.targets.rebroadcast_in_flight_event_id()
    }

    pub(in crate::ui::workspace) fn sync_from_workspace(
        &mut self,
        page: Option<&ClipboardWorkspaceViewState>,
        window: &mut Window,
        cx: &mut Context<WorkspaceView>,
    ) {
        let Some(page) = page else {
            self.inputs.sync_read_record(None, window, cx);
            return;
        };

        if let Some(record) = page.latest_record.clone() {
            self.history.promote_record(record);
        }
        self.targets
            .retain_connected_sessions(&page.session_targets);

        let selected_record = self.selected_record(page.latest_record.as_ref());
        self.inputs
            .sync_read_record(selected_record.as_ref(), window, cx);
    }

    pub(in crate::ui::workspace) fn can_load_more(&self) -> bool {
        self.history.can_load_more()
    }

    pub(in crate::ui::workspace) fn begin_history_load(&mut self, initial: bool) -> bool {
        self.history.begin_load(initial)
    }

    pub(in crate::ui::workspace) fn finish_history_load(&mut self, page: ClipboardHistoryPage) {
        self.history.finish_load(page);
    }

    pub(in crate::ui::workspace) fn fail_history_load(&mut self, message: String) {
        self.history.mark_load_failed();
        self.feedback = Some(message);
    }

    pub(in crate::ui::workspace) fn select_latest(&mut self) {
        self.history.select_latest();
        self.editor.clear_session();
        self.feedback = None;
    }

    pub(in crate::ui::workspace) fn select_history(&mut self, event_id: EventId) {
        self.history.select_history(event_id);
        self.editor.clear_session();
        self.feedback = None;
    }

    pub(in crate::ui::workspace) fn has_cached_record(&self, event_id: EventId) -> bool {
        self.history.has_cached_record(event_id)
    }

    pub(in crate::ui::workspace) fn selected_record(
        &self,
        latest_record: Option<&ClipboardRecord>,
    ) -> Option<ClipboardRecord> {
        self.history.selected_record(latest_record)
    }

    pub(in crate::ui::workspace) fn begin_edit_session(
        &mut self,
        record: &ClipboardRecord,
        window: &mut Window,
        cx: &mut Context<WorkspaceView>,
    ) {
        self.editor.begin_session(record);
        self.inputs
            .set_edit_text(record.content.clone(), window, cx);
    }

    pub(in crate::ui::workspace) fn clear_edit_session(
        &mut self,
        window: &mut Window,
        cx: &mut Context<WorkspaceView>,
    ) {
        self.editor.clear_session();
        self.inputs.clear_edit_text(window, cx);
    }

    pub(in crate::ui::workspace) fn edit_text(&self, cx: &Context<WorkspaceView>) -> String {
        self.inputs.edit_text(cx)
    }

    pub(in crate::ui::workspace) fn edit_bytes(&self, cx: &Context<WorkspaceView>) -> usize {
        self.inputs.edit_bytes(cx)
    }

    pub(in crate::ui::workspace) fn is_edit_dirty(&self, cx: &Context<WorkspaceView>) -> bool {
        self.editor.is_dirty(&self.inputs.edit_text(cx))
    }

    pub(in crate::ui::workspace) fn can_submit_edit(
        &self,
        max_text_bytes: usize,
        cx: &Context<WorkspaceView>,
    ) -> bool {
        self.editor
            .can_submit(&self.inputs.edit_text(cx), max_text_bytes)
    }

    pub(in crate::ui::workspace) fn start_submit(&mut self) {
        self.editor.start_submit();
        self.feedback = Some("Saving edited clipboard content as a new record.".to_string());
    }

    pub(in crate::ui::workspace) fn finish_submit(
        &mut self,
        window: &mut Window,
        cx: &mut Context<WorkspaceView>,
        message: String,
    ) {
        self.history.select_latest();
        self.editor.finish_submit();
        self.inputs.clear_edit_text(window, cx);
        self.feedback = Some(message);
    }

    pub(in crate::ui::workspace) fn fail_submit(&mut self, message: String) {
        self.editor.fail_submit();
        self.feedback = Some(message);
    }

    pub(in crate::ui::workspace) fn start_adopt(&mut self, event_id: EventId) {
        self.targets.start_adopt(event_id);
        self.feedback = Some("Adopting selected committed record locally.".to_string());
    }

    pub(in crate::ui::workspace) fn finish_adopt(&mut self, event_id: EventId, message: String) {
        self.targets.finish_adopt(event_id);
        self.feedback = Some(message);
    }

    pub(in crate::ui::workspace) fn start_rebroadcast(&mut self, event_id: EventId) {
        self.targets.start_rebroadcast(event_id);
        self.feedback = Some("Rebroadcasting selected record to connected sessions.".to_string());
    }

    pub(in crate::ui::workspace) fn finish_rebroadcast(
        &mut self,
        event_id: EventId,
        message: String,
    ) {
        self.targets.finish_rebroadcast(event_id);
        self.feedback = Some(message);
    }

    pub(in crate::ui::workspace) fn cache_record(&mut self, record: ClipboardRecord) {
        self.history.cache_record(record);
    }

    pub(in crate::ui::workspace) fn set_broadcast_scope(&mut self, scope: ClipboardBroadcastScope) {
        self.targets.set_broadcast_scope(scope);
    }

    pub(in crate::ui::workspace) fn toggle_session_target(&mut self, session_id: SessionId) {
        self.targets.toggle_session_target(session_id);
    }

    pub(in crate::ui::workspace) fn rebroadcast_target(&self) -> SessionTarget {
        self.targets.rebroadcast_target()
    }
}
