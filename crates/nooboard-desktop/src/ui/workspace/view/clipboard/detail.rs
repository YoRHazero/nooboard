use super::*;
use gpui_component::StyledExt;

impl WorkspaceView {
    pub(super) fn clipboard_detail_panel(
        &self,
        active_item: &ClipboardTextItem,
        cx: &mut Context<Self>,
    ) -> Div {
        div()
            .flex_1()
            .min_w(px(0.0))
            .v_flex()
            .gap(px(16.0))
            .child(self.clipboard_info_panel(active_item))
            .child(self.clipboard_text_panel(active_item))
            .child(self.clipboard_actions_panel(active_item, cx))
    }

    fn clipboard_info_panel(&self, active_item: &ClipboardTextItem) -> Div {
        let mut panel = div()
            .v_flex()
            .gap(px(10.0))
            .p(px(16.0))
            .bg(theme::bg_panel())
            .border_1()
            .border_color(theme::border_base())
            .rounded(px(20.0))
            .shadow_xs()
            .child(
                div()
                    .h_flex()
                    .justify_between()
                    .items_center()
                    .gap(px(14.0))
                    .child(
                        div()
                            .h_flex()
                            .items_center()
                            .gap(px(10.0))
                            .min_w(px(0.0))
                            .child(
                                div()
                                    .h_flex()
                                    .gap(px(8.0))
                                    .items_center()
                                    .child(self.clipboard_badge(
                                        self.clipboard_origin_label(active_item),
                                        self.clipboard_item_accent(active_item),
                                    ))
                                    .child(self.clipboard_badge(
                                        self.clipboard_residency_label(active_item).to_string(),
                                        if active_item.residency == ClipboardTextResidency::History
                                        {
                                            theme::accent_amber()
                                        } else {
                                            theme::accent_blue()
                                        },
                                    )),
                            )
                            .child(
                                div()
                                    .text_size(px(15.0))
                                    .font_semibold()
                                    .text_color(theme::fg_primary())
                                    .truncate()
                                    .child(active_item.device_id.clone()),
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(theme::fg_muted())
                                    .truncate()
                                    .child(format!(
                                        "{} · event {}",
                                        active_item.recorded_at_label,
                                        self.clipboard_short_event_id(active_item.event_id)
                                    )),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(theme::fg_muted())
                            .child(format!(
                                "{} target{}",
                                self.clipboard_page.selected_target_count(),
                                if self.clipboard_page.selected_target_count() == 1 {
                                    ""
                                } else {
                                    "s"
                                }
                            )),
                    ),
            );

        if let Some(message) = self.clipboard_page.action_feedback() {
            let feedback = message.to_owned();
            panel = panel.child(
                div()
                    .text_size(px(11.0))
                    .text_color(theme::fg_muted())
                    .line_clamp(1)
                    .text_ellipsis()
                    .child(feedback),
            );
        }

        panel
    }

    fn clipboard_text_panel(&self, active_item: &ClipboardTextItem) -> Div {
        div()
            .flex_1()
            .min_h(px(CLIPBOARD_TEXT_PANEL_MIN_HEIGHT))
            .v_flex()
            .gap(px(12.0))
            .p(px(22.0))
            .bg(theme::bg_panel())
            .border_1()
            .border_color(theme::border_base())
            .rounded(px(24.0))
            .shadow_xs()
            .child(
                div()
                    .text_size(px(18.0))
                    .font_semibold()
                    .text_color(theme::fg_primary())
                    .child("Content".to_string()),
            )
            .child(
                div().flex_1().min_h(px(0.0)).overflow_y_scrollbar().child(
                    div()
                        .w_full()
                        .p(px(18.0))
                        .bg(theme::bg_console())
                        .border_1()
                        .border_color(theme::border_soft())
                        .rounded(px(18.0))
                        .text_size(px(14.0))
                        .text_color(theme::fg_primary())
                        .whitespace_normal()
                        .child(active_item.content.clone()),
                ),
            )
    }

    fn clipboard_actions_panel(
        &self,
        active_item: &ClipboardTextItem,
        cx: &mut Context<Self>,
    ) -> Div {
        let selected_targets = self.clipboard_page.selected_target_count();
        let broadcast_disabled = !active_item.can_broadcast() || selected_targets == 0;
        let write_disabled = !active_item.can_write_to_clipboard();
        let store_disabled = !active_item.can_store();
        let delete_disabled = !active_item.can_delete();
        let store_item = active_item.clone();
        let write_item = active_item.clone();
        let broadcast_item = active_item.clone();
        let delete_event_id = active_item.event_id;
        let write_tooltip = if write_disabled {
            "Remote live or history only.".to_string()
        } else {
            "Write this text to local clipboard.".to_string()
        };
        let broadcast_tooltip = if !active_item.can_broadcast() {
            "Local live or history only.".to_string()
        } else if selected_targets == 0 {
            "Select at least one connected target.".to_string()
        } else {
            "Broadcast to selected targets.".to_string()
        };
        let store_tooltip = if store_disabled {
            "Remote live only.".to_string()
        } else {
            "Store this remote text into history.".to_string()
        };
        let delete_tooltip = if delete_disabled {
            "History only.".to_string()
        } else {
            "Delete this history item.".to_string()
        };

        div()
            .v_flex()
            .gap(px(14.0))
            .p(px(22.0))
            .bg(theme::bg_panel())
            .border_1()
            .border_color(theme::border_base())
            .rounded(px(24.0))
            .shadow_xs()
            .child(
                div()
                    .text_size(px(18.0))
                    .font_semibold()
                    .text_color(theme::fg_primary())
                    .child("Actions".to_string()),
            )
            .child(
                div()
                    .h_flex()
                    .gap(px(10.0))
                    .flex_wrap()
                    .child(
                        self.clipboard_action_with_tooltip(
                            "clipboard-action-write-tooltip-shell",
                            self.clipboard_action_button(
                                "clipboard-action-write",
                                "Write",
                                theme::accent_blue(),
                                write_disabled,
                                cx,
                            )
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    this.set_clipboard_feedback(format!(
                                        "Wrote {} to the clipboard.",
                                        this.clipboard_short_event_id(write_item.event_id)
                                    ));
                                    cx.notify();
                                },
                            )),
                            Some(write_tooltip),
                        ),
                    )
                    .child(
                        self.clipboard_action_with_tooltip(
                            "clipboard-action-broadcast-tooltip-shell",
                            self.clipboard_action_button(
                                "clipboard-action-broadcast",
                                "Broadcast",
                                theme::accent_cyan(),
                                broadcast_disabled,
                                cx,
                            )
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    let count = this.clipboard_page.selected_target_count();
                                    this.set_clipboard_feedback(format!(
                                        "Queued {} to {} target{}.",
                                        this.clipboard_short_event_id(broadcast_item.event_id),
                                        count,
                                        if count == 1 { "" } else { "s" }
                                    ));
                                    cx.notify();
                                },
                            )),
                            Some(broadcast_tooltip),
                        ),
                    )
                    .child(
                        self.clipboard_action_with_tooltip(
                            "clipboard-action-store-tooltip-shell",
                            self.clipboard_action_button(
                                "clipboard-action-store",
                                "Store",
                                theme::accent_amber(),
                                store_disabled,
                                cx,
                            )
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    this.store_remote_clipboard_item(store_item.clone(), cx);
                                },
                            )),
                            Some(store_tooltip),
                        ),
                    )
                    .child(
                        self.clipboard_action_with_tooltip(
                            "clipboard-action-delete-tooltip-shell",
                            self.clipboard_action_button(
                                "clipboard-action-delete",
                                "Delete",
                                theme::accent_rose(),
                                delete_disabled,
                                cx,
                            )
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    this.delete_history_clipboard_item(delete_event_id, cx);
                                },
                            )),
                            Some(delete_tooltip),
                        ),
                    ),
            )
    }

    pub(super) fn clipboard_action_button(
        &self,
        id: &'static str,
        label: &str,
        accent: Hsla,
        disabled: bool,
        cx: &mut Context<Self>,
    ) -> Button {
        let variant = ButtonCustomVariant::new(cx)
            .color(accent.opacity(0.10))
            .foreground(theme::fg_primary())
            .hover(accent.opacity(0.34))
            .active(accent.opacity(0.48))
            .shadow(false);

        Button::new(id)
            .custom(variant)
            .rounded(px(999.0))
            .border_1()
            .border_color(accent.opacity(0.38))
            .disabled(disabled)
            .child(
                div()
                    .text_color(theme::fg_primary())
                    .font_semibold()
                    .child(label.to_string()),
            )
    }

    fn clipboard_action_with_tooltip(
        &self,
        id: &'static str,
        button: Button,
        tooltip: Option<String>,
    ) -> AnyElement {
        match tooltip {
            Some(text) => div()
                .id(id)
                .tooltip(move |window: &mut Window, cx| {
                    Self::clipboard_themed_tooltip(text.clone(), window, cx)
                })
                .child(button)
                .into_any_element(),
            None => button.into_any_element(),
        }
    }
}
