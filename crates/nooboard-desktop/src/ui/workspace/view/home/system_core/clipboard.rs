use super::*;

impl WorkspaceView {
    fn clipboard_snapshot_card(
        &self,
        snapshot: &ClipboardSnapshot,
        accent: Hsla,
        show_copy_action: bool,
    ) -> Div {
        let copy_id = if snapshot.origin == ClipboardOrigin::Remote {
            1usize
        } else {
            0usize
        };

        div()
            .v_flex()
            .gap(px(12.0))
            .flex_1()
            .min_h(px(0.0))
            .p(px(14.0))
            .bg(theme::bg_panel_alt())
            .border_1()
            .border_color(accent.opacity(0.24))
            .rounded(px(20.0))
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
                                    .child(div().size(px(8.0)).rounded(px(999.0)).bg(accent))
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .font_semibold()
                                            .text_color(theme::fg_primary())
                                            .truncate()
                                            .child(snapshot.device_id.clone()),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(10.0))
                                    .text_color(theme::fg_muted())
                                    .child(snapshot.captured_at_label.clone()),
                            ),
                    )
                    .when(show_copy_action, |this| {
                        this.child(
                            Clipboard::new(("system-core-clipboard-copy", copy_id))
                                .value(snapshot.content.clone()),
                        )
                    }),
            )
            .child(
                div()
                    .text_size(px(15.0))
                    .font_semibold()
                    .text_color(theme::fg_primary())
                    .line_clamp(4)
                    .text_ellipsis()
                    .child(snapshot.content.clone()),
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
            .v_flex()
            .gap(px(12.0))
            .p(px(14.0))
            .bg(theme::bg_console())
            .border_1()
            .border_color(theme::border_soft())
            .rounded(px(24.0))
            .child(
                div()
                    .h_flex()
                    .justify_between()
                    .items_center()
                    .gap(px(12.0))
                    .child(
                        div()
                            .h_flex()
                            .items_center()
                            .gap(px(10.0))
                            .child(
                                div()
                                    .size(px(32.0))
                                    .rounded(px(11.0))
                                    .bg(theme::accent_blue().opacity(0.12))
                                    .border_1()
                                    .border_color(theme::accent_blue().opacity(0.24))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(
                                        Icon::new(IconName::Copy)
                                            .size(px(15.0))
                                            .text_color(theme::accent_blue()),
                                    ),
                            )
                            .child(
                                div()
                                    .v_flex()
                                    .gap(px(2.0))
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .font_semibold()
                                            .text_color(theme::fg_primary())
                                            .child("Clipboard Relay"),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(10.0))
                                            .text_color(theme::fg_muted())
                                            .child(
                                                if latest_snapshot.origin == ClipboardOrigin::Remote
                                                {
                                                    "showing latest remote text"
                                                } else {
                                                    "showing latest local clipboard"
                                                },
                                            ),
                                    ),
                            ),
                    )
                    .child(div().size(px(8.0)).rounded(px(999.0)).bg(latest_accent)),
            )
            .child(self.clipboard_snapshot_card(latest_snapshot, latest_accent, show_copy_action))
            .child(
                div()
                    .text_size(px(10.0))
                    .text_color(theme::fg_muted())
                    .child(if latest_snapshot.origin == ClipboardOrigin::Remote {
                        if self.auto_bridge_remote_text {
                            "auto-forward enabled"
                        } else {
                            "manual adopt mode"
                        }
                    } else if self.auto_bridge_remote_text {
                        "auto-forward enabled, waiting for newer remote text"
                    } else {
                        "manual adopt mode, local clipboard is still newest"
                    }),
            )
    }
}
