use gpui::prelude::FluentBuilder as _;
use gpui::{AnyElement, InteractiveElement, StatefulInteractiveElement};
use gpui_component::IconName;
use gpui_component::StyledExt;
use nooboard_app::ClipboardRecord;

use super::components::{clipboard_icon_action_button, clipboard_themed_tooltip};
use super::snapshot::{
    ClipboardHistoryRowSnapshot, clipboard_record_preview, clipboard_record_time_label,
};
use super::*;

impl WorkspaceView {
    pub(super) fn clipboard_history_panel(
        &self,
        snapshot: &ClipboardSnapshot,
        cx: &mut Context<Self>,
    ) -> Div {
        let history_rows: Vec<_> = snapshot
            .history_rows
            .iter()
            .enumerate()
            .map(|(index, row)| self.clipboard_history_item(index, row, cx))
            .collect();

        clipboard_panel_shell()
            .rounded(px(24.0))
            .w(px(CLIPBOARD_HISTORY_WIDTH))
            .flex_shrink_0()
            .v_flex()
            .gap(px(16.0))
            .p(px(20.0))
            .child(clipboard_panel_header(
                "Committed History",
                format!("{} loaded", snapshot.loaded_history_count),
            ))
            .child(div().v_flex().gap(px(10.0)).when_some(
                snapshot.latest_record.clone(),
                |this, latest_record| {
                    this.child(
                        div()
                            .v_flex()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .text_size(px(10.0))
                                    .font_semibold()
                                    .text_color(theme::fg_muted())
                                    .child("Latest committed".to_string()),
                            )
                            .child(self.clipboard_latest_item(
                                latest_record,
                                snapshot.latest_selected,
                                cx,
                            )),
                    )
                },
            ))
            .child(div().flex_1().min_h(px(0.0)).overflow_y_scrollbar().child(
                if history_rows.is_empty() {
                    div()
                        .w_full()
                        .py(px(18.0))
                        .text_size(px(11.0))
                        .text_color(theme::fg_muted())
                        .child("No earlier committed records are loaded yet.")
                        .into_any_element()
                } else {
                    div()
                        .w_full()
                        .v_flex()
                        .gap(px(12.0))
                        .children(history_rows)
                        .into_any_element()
                },
            ))
            .child(
                div().h_flex().justify_end().child(
                    div()
                        .id("clipboard-history-load-more-tooltip")
                        .tooltip(|window, cx| {
                            clipboard_themed_tooltip(
                                "Load more committed clipboard records".to_string(),
                                window,
                                cx,
                            )
                        })
                        .child(
                            clipboard_icon_action_button(
                                "clipboard-history-load-more",
                                IconName::ChevronDown,
                                theme::accent_amber(),
                                !snapshot.can_load_more,
                                cx,
                            )
                            .loading(
                                snapshot.history_load_state
                                    == page_state::ClipboardHistoryLoadState::LoadingMore,
                            )
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.load_more_clipboard_history(cx);
                            })),
                        ),
                ),
            )
    }

    fn clipboard_latest_item(
        &self,
        record: ClipboardRecord,
        selected: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let accent = self.clipboard_source_accent(record.source);
        let event_id = record.event_id;

        clipboard_history_item_shell(selected, accent)
            .id(("clipboard-latest-item", 0usize))
            .cursor_pointer()
            .hover(|this| {
                this.bg(theme::bg_panel_alt())
                    .border_color(theme::border_strong())
            })
            .active(|this| this.bg(theme::bg_panel()))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.request_clipboard_select_latest(window, cx);
            }))
            .child(clipboard_history_item_body(
                record.origin_device_id.clone(),
                format!(
                    "{} · event {}",
                    clipboard_record_time_label(&record),
                    self.clipboard_short_event_id(event_id)
                ),
                clipboard_badge(clipboard_source_label(record.source), accent),
                clipboard_record_preview(&record.content, 92),
            ))
            .into_any_element()
    }

    fn clipboard_history_item(
        &self,
        index: usize,
        row: &ClipboardHistoryRowSnapshot,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let accent = self.clipboard_source_accent(row.record.source);
        let event_id = row.record.event_id;

        clipboard_history_item_shell(row.selected, accent)
            .id(("clipboard-history-item", index))
            .cursor_pointer()
            .hover(|this| {
                this.bg(theme::bg_panel_alt())
                    .border_color(theme::border_strong())
            })
            .active(|this| this.bg(theme::bg_panel()))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.request_clipboard_select_history(event_id, window, cx);
            }))
            .child(clipboard_history_item_body(
                row.record.origin_device_id.clone(),
                format!(
                    "{} · event {}",
                    clipboard_record_time_label(&row.record),
                    self.clipboard_short_event_id(event_id)
                ),
                clipboard_badge(clipboard_source_label(row.record.source), accent),
                clipboard_record_preview(&row.record.content, 92),
            ))
            .into_any_element()
    }
}
