use super::*;
use gpui::StatefulInteractiveElement;

impl WorkspaceView {
    pub(super) fn clipboard_history_panel(&self, cx: &mut Context<Self>) -> Div {
        let history_rows: Vec<_> = self
            .clipboard_page
            .history_items()
            .iter()
            .enumerate()
            .map(|(index, item)| self.clipboard_history_item(index, item, cx))
            .collect();

        clipboard_panel_shell()
            .rounded(px(24.0))
            .w(px(CLIPBOARD_HISTORY_WIDTH))
            .flex_shrink_0()
            .v_flex()
            .gap(px(16.0))
            .p(px(20.0))
            .child(clipboard_panel_header(
                "Stored History",
                format!("{} loaded", self.clipboard_page.history_items().len()),
            ))
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.0))
                    .overflow_y_scrollbar()
                    .child(div().w_full().v_flex().gap(px(12.0)).children(history_rows)),
            )
            .child(
                clipboard_action_button(
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

        clipboard_history_item_shell(selected, accent)
            .id(("clipboard-history-item", index))
            .cursor_pointer()
            .hover(|this| {
                this.bg(theme::bg_panel_alt())
                    .border_color(theme::border_strong())
            })
            .active(|this| this.bg(theme::bg_panel()))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.toggle_clipboard_history_selection(event_id, cx);
            }))
            .child(clipboard_history_item_body(
                item.device_id.clone(),
                item.recorded_at_label.clone(),
                clipboard_badge(self.clipboard_origin_label(item), accent),
                item.preview_text(92),
            ))
            .into_any_element()
    }
}
