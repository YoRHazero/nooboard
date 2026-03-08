use super::*;

impl WorkspaceView {
    fn clipboard_copy_action(&self, item: &ClipboardTextItem, accent: Hsla) -> impl IntoElement {
        let copy_id = if item.origin == ClipboardTextOrigin::Remote {
            1usize
        } else {
            0usize
        };

        clipboard_copy_action_shell(accent)
            .id(("system-core-clipboard-copy-shell", copy_id))
            .hover(|this| {
                this.bg(theme::bg_panel_highlight())
                    .border_color(accent.opacity(0.3))
            })
            .active(|this| {
                this.bg(theme::bg_panel())
                    .border_color(accent.opacity(0.24))
            })
            .tooltip(move |window: &mut Window, cx| {
                Self::themed_tooltip("Copy original clipboard text".into(), window, cx)
            })
            .child(
                Clipboard::new(("system-core-clipboard-copy", copy_id)).value(item.content.clone()),
            )
    }

    fn clipboard_read_board(
        &self,
        item: &ClipboardTextItem,
        accent: Hsla,
        show_copy_action: bool,
    ) -> Div {
        clipboard_read_board(
            item.device_id.clone(),
            item.recorded_at_label.clone(),
            accent,
            if show_copy_action {
                self.clipboard_copy_action(item, accent).into_any_element()
            } else {
                clipboard_copy_placeholder(accent).into_any_element()
            },
            item.content.clone(),
        )
    }

    pub(super) fn clipboard_panel(&self) -> Div {
        let clipboard = &self.state.app.clipboard;
        let core = &self.state.app.system_core;
        let latest_item = clipboard.latest_live_item();
        let latest_accent = if latest_item.origin == ClipboardTextOrigin::Remote {
            theme::accent_blue()
        } else {
            theme::accent_green()
        };
        let show_copy_action = latest_item.origin == ClipboardTextOrigin::Remote
            && !self.auto_bridge_remote_text
            && latest_item.device_id != core.local_device_id;

        div()
            .w(px(CLIPBOARD_PANEL_WIDTH))
            .flex_shrink_0()
            .h(px(CLIPBOARD_PANEL_HEIGHT))
            .bg(theme::bg_console())
            .border_1()
            .border_color(theme::border_soft())
            .rounded(px(24.0))
            .shadow_xs()
            .child(self.clipboard_read_board(latest_item, latest_accent, show_copy_action))
    }
}
