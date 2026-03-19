use gpui::{Context, Window};
use nooboard_core::ClipboardHistoryDirection;

use crate::{ui::workspace::WorkspaceView, workspace::view_state::ClipboardWorkspaceViewState};

impl WorkspaceView {
    pub(in crate::ui::workspace) fn sync_clipboard_from_workspace(
        &mut self,
        page: Option<&ClipboardWorkspaceViewState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.clipboard.sync_from_workspace(page, window, cx);
    }

    pub(in crate::ui::workspace) fn bootstrap_clipboard_history_if_needed(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        if self.clipboard.history_bootstrapped()
            || !self
                .clipboard
                .begin_history_load(ClipboardHistoryDirection::Older, true)
        {
            return;
        }

        self.request_clipboard_history_page(ClipboardHistoryDirection::Older, None, cx);
    }
}
