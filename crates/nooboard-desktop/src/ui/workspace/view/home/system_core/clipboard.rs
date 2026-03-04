use super::*;

impl WorkspaceView {
    fn clipboard_copy_action(
        &self,
        snapshot: &ClipboardSnapshot,
        accent: Hsla,
    ) -> impl IntoElement {
        let copy_id = if snapshot.origin == ClipboardOrigin::Remote {
            1usize
        } else {
            0usize
        };

        div()
            .id(("system-core-clipboard-copy-shell", copy_id))
            .size(px(34.0))
            .cursor_pointer()
            .rounded(px(12.0))
            .bg(theme::bg_panel_alt())
            .border_1()
            .border_color(accent.opacity(0.22))
            .flex()
            .items_center()
            .justify_center()
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
                Clipboard::new(("system-core-clipboard-copy", copy_id))
                    .value(snapshot.content.clone()),
            )
    }

    fn clipboard_copy_placeholder(&self, accent: Hsla) -> Div {
        div()
            .size(px(34.0))
            .rounded(px(12.0))
            .bg(theme::bg_panel_alt())
            .border_1()
            .border_color(theme::border_soft())
            .flex()
            .items_center()
            .justify_center()
            .opacity(0.56)
            .child(
                Icon::new(IconName::Copy)
                    .size(px(15.0))
                    .text_color(accent.opacity(0.9)),
            )
    }

    fn clipboard_read_board(
        &self,
        snapshot: &ClipboardSnapshot,
        accent: Hsla,
        show_copy_action: bool,
    ) -> Div {
        div()
            .relative()
            .v_flex()
            .size_full()
            .overflow_hidden()
            .child(
                div()
                    .v_flex()
                    .size_full()
                    .gap(px(16.0))
                    .p(px(18.0))
                    .child(
                        div()
                            .h_flex()
                            .justify_between()
                            .items_start()
                            .gap(px(12.0))
                            .child(
                                div()
                                    .v_flex()
                                    .gap(px(6.0))
                                    .child(
                                        div()
                                            .h_flex()
                                            .items_center()
                                            .gap(px(10.0))
                                            .child(
                                                div().size(px(8.0)).rounded(px(999.0)).bg(accent),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(13.0))
                                                    .font_semibold()
                                                    .text_color(theme::fg_primary())
                                                    .truncate()
                                                    .child(snapshot.device_id.clone()),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(10.0))
                                            .font_semibold()
                                            .text_color(theme::fg_muted())
                                            .child(snapshot.captured_at_label.clone()),
                                    ),
                            )
                            .child(if show_copy_action {
                                self.clipboard_copy_action(snapshot, accent)
                                    .into_any_element()
                            } else {
                                self.clipboard_copy_placeholder(accent).into_any_element()
                            }),
                    )
                    .child(
                        div()
                            .w_full()
                            .h(px(1.0))
                            .bg(theme::border_soft().opacity(0.94)),
                    )
                    .child(
                        div().relative().flex_1().min_h(px(0.0)).child(
                            div()
                                .absolute()
                                .top(px(0.0))
                                .left(px(0.0))
                                .right(px(0.0))
                                .bottom(px(0.0))
                                .text_size(px(14.0))
                                .text_color(theme::fg_primary())
                                .line_clamp(12)
                                .text_ellipsis()
                                .child(snapshot.content.clone()),
                        ),
                    ),
            )
    }

    pub(super) fn clipboard_panel(&self) -> Div {
        let core = &self.state.app.system_core;
        let local_snapshot = &core.local_clipboard;
        let latest_snapshot = match core.latest_remote_clipboard.as_ref() {
            Some(snapshot) if snapshot.updated_at_order > local_snapshot.updated_at_order => {
                snapshot
            }
            _ => local_snapshot,
        };
        let latest_accent = if latest_snapshot.origin == ClipboardOrigin::Remote {
            theme::accent_blue()
        } else {
            theme::accent_green()
        };
        let show_copy_action = latest_snapshot.origin == ClipboardOrigin::Remote
            && !self.auto_bridge_remote_text
            && latest_snapshot.device_id != core.local_device_id;

        div()
            .w(px(CLIPBOARD_PANEL_WIDTH))
            .flex_shrink_0()
            .h(px(CLIPBOARD_PANEL_HEIGHT))
            .bg(theme::bg_console())
            .border_1()
            .border_color(theme::border_soft())
            .rounded(px(24.0))
            .shadow_xs()
            .child(self.clipboard_read_board(latest_snapshot, latest_accent, show_copy_action))
    }
}
