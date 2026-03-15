use gpui::{AppContext as _, Context, Entity, Window};
use gpui_component::input::InputState;
use nooboard_core::{ClipboardRecord, EventId};

use crate::ui::workspace::WorkspaceView;

pub(super) struct ClipboardInputs {
    read_input: Entity<InputState>,
    read_event_id: Option<EventId>,
    edit_input: Entity<InputState>,
}

impl ClipboardInputs {
    pub(super) fn new(window: &mut Window, cx: &mut Context<WorkspaceView>) -> Self {
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
                .placeholder("Edit a committed clipboard record and save it as a new event.")
        });

        Self {
            read_input,
            read_event_id: None,
            edit_input,
        }
    }

    pub(super) fn read_input(&self) -> Entity<InputState> {
        self.read_input.clone()
    }

    pub(super) fn edit_input(&self) -> Entity<InputState> {
        self.edit_input.clone()
    }

    pub(super) fn sync_read_record(
        &mut self,
        record: Option<&ClipboardRecord>,
        window: &mut Window,
        cx: &mut Context<WorkspaceView>,
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

    pub(super) fn set_edit_text(
        &mut self,
        value: String,
        window: &mut Window,
        cx: &mut Context<WorkspaceView>,
    ) {
        self.edit_input.update(cx, |input, cx| {
            input.set_value(value, window, cx);
        });
    }

    pub(super) fn clear_edit_text(&mut self, window: &mut Window, cx: &mut Context<WorkspaceView>) {
        self.set_edit_text(String::new(), window, cx);
    }

    pub(super) fn edit_text(&self, cx: &Context<WorkspaceView>) -> String {
        self.edit_input.read(cx).value().to_string()
    }

    pub(super) fn edit_bytes(&self, cx: &Context<WorkspaceView>) -> usize {
        self.edit_text(cx).len()
    }
}
