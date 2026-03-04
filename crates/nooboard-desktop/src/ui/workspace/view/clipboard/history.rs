use super::*;

impl WorkspaceView {
    pub(super) fn clipboard_history_panel(&self, cx: &mut Context<Self>) -> Div {
        let history_rows: Vec<_> = self
            .clipboard_page
            .history_items()
            .iter()
            .enumerate()
            .map(|(index, item)| self.clipboard_history_item(index, item, cx))
            .collect();

        div()
            .w(px(CLIPBOARD_HISTORY_WIDTH))
            .flex_shrink_0()
            .v_flex()
            .gap(px(16.0))
            .p(px(20.0))
            .bg(theme::bg_panel())
            .border_1()
            .border_color(theme::border_base())
            .rounded(px(24.0))
            .shadow_xs()
            .child(
                div()
                    .h_flex()
                    .justify_between()
                    .items_center()
                    .gap(px(16.0))
                    .child(
                        div()
                            .text_size(px(16.0))
                            .font_semibold()
                            .text_color(theme::fg_primary())
                            .child("Stored History"),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(theme::fg_muted())
                            .child(format!(
                                "{} loaded",
                                self.clipboard_page.history_items().len()
                            )),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.0))
                    .overflow_y_scrollbar()
                    .child(div().v_flex().gap(px(12.0)).children(history_rows)),
            )
            .child(
                self.clipboard_action_button(
                    "clipboard-history-load-more",
                    self.clipboard_page.load_more_label(),
                    theme::accent_amber(),
                    !self.clipboard_page.can_load_more(),
                    cx,
                )
                .w_full()
                .on_click(cx.listener(|this, _, _, cx| {
                    this.load_more_clipboard_history(cx);
                })),
            )
    }

    fn clipboard_history_item(
        &self,
        index: usize,
        item: &ClipboardTextItem,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let selected = self.clipboard_page.is_history_selected(item.event_id);
        let accent = self.clipboard_item_accent(item);
        let event_id = item.event_id;

        div()
            .id(("clipboard-history-item", index))
            .w_full()
            .cursor_pointer()
            .px(px(14.0))
            .py(px(14.0))
            .bg(if selected {
                theme::bg_panel_highlight()
            } else {
                theme::bg_console()
            })
            .border_1()
            .border_color(if selected {
                accent.opacity(0.34)
            } else {
                theme::border_soft()
            })
            .rounded(px(20.0))
            .shadow_xs()
            .hover(|this| {
                this.bg(theme::bg_panel_alt())
                    .border_color(theme::border_strong())
            })
            .active(|this| this.bg(theme::bg_panel()))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.toggle_clipboard_history_selection(event_id, cx);
            }))
            .child(
                div()
                    .v_flex()
                    .gap(px(10.0))
                    .child(
                        div()
                            .h_flex()
                            .items_center()
                            .justify_between()
                            .gap(px(12.0))
                            .child(
                                div()
                                    .v_flex()
                                    .gap(px(5.0))
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .font_semibold()
                                            .text_color(theme::fg_primary())
                                            .child(item.device_id.clone()),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(10.0))
                                            .font_semibold()
                                            .text_color(theme::fg_muted())
                                            .child(item.recorded_at_label.clone()),
                                    ),
                            )
                            .child(self.clipboard_badge(self.clipboard_origin_label(item), accent)),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(theme::fg_secondary())
                            .line_clamp(2)
                            .text_ellipsis()
                            .child(item.preview_text(92)),
                    ),
            )
            .into_any_element()
    }
}
