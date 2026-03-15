use gpui::{
    Context, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled,
    div, px,
};
use gpui_component::StyledExt;

use crate::ui::theme;

use super::{
    CLIPBOARD_HISTORY_WIDTH,
    components::{clipboard_badge, clipboard_history_item_shell, clipboard_panel_shell},
    state::ClipboardHistoryLoadState,
    view_state::{
        ClipboardHistoryRowViewState, ClipboardPageViewState, clipboard_record_preview,
        clipboard_record_time_label, clipboard_short_event_id, clipboard_source_label,
    },
};
use crate::ui::workspace::WorkspaceView;

impl WorkspaceView {
    pub(super) fn clipboard_history_panel(
        &self,
        snapshot: &ClipboardPageViewState,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        clipboard_panel_shell()
            .w(px(CLIPBOARD_HISTORY_WIDTH))
            .min_w(px(CLIPBOARD_HISTORY_WIDTH))
            .flex_shrink_0()
            .v_flex()
            .gap(px(16.0))
            .p(px(20.0))
            .child(
                div()
                    .v_flex()
                    .gap(px(4.0))
                    .child(
                        div()
                            .text_size(px(14.0))
                            .font_semibold()
                            .text_color(theme::fg_primary())
                            .child("Committed History"),
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(theme::fg_muted())
                            .child(format!("{} loaded", snapshot.loaded_history_count)),
                    ),
            )
            .children(
                snapshot
                    .latest_record
                    .clone()
                    .into_iter()
                    .map(|latest_record| {
                        let accent = self.clipboard_source_accent(latest_record.source);
                        let record = latest_record.clone();
                        div()
                            .v_flex()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .text_size(px(10.0))
                                    .font_semibold()
                                    .text_color(theme::fg_muted())
                                    .child("Latest committed"),
                            )
                            .child(
                                div()
                                    .id("clipboard-latest-item")
                                    .cursor_pointer()
                                    .hover(|this| this.bg(theme::bg_panel_alt()))
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.request_clipboard_select_latest(window, cx);
                                    }))
                                    .child(
                                        clipboard_history_item_shell(
                                            snapshot.latest_selected,
                                            accent,
                                        )
                                        .child(
                                            div()
                                                .v_flex()
                                                .gap(px(8.0))
                                                .child(
                                                    div()
                                                        .h_flex()
                                                        .justify_between()
                                                        .gap(px(10.0))
                                                        .child(
                                                            div()
                                                                .text_size(px(12.0))
                                                                .font_semibold()
                                                                .text_color(theme::fg_primary())
                                                                .child(
                                                                    record.origin_device_id.clone(),
                                                                ),
                                                        )
                                                        .child(clipboard_badge(
                                                            clipboard_source_label(record.source),
                                                            accent,
                                                        )),
                                                )
                                                .child(
                                                    div()
                                                        .text_size(px(11.0))
                                                        .text_color(theme::fg_muted())
                                                        .child(format!(
                                                            "{} · event {}",
                                                            clipboard_record_time_label(&record),
                                                            clipboard_short_event_id(&record)
                                                        )),
                                                )
                                                .child(
                                                    div()
                                                        .text_size(px(12.0))
                                                        .line_height(px(18.0))
                                                        .text_color(theme::fg_primary())
                                                        .child(clipboard_record_preview(
                                                            &record.content,
                                                            92,
                                                        )),
                                                ),
                                        ),
                                    ),
                            )
                            .into_any_element()
                    }),
            )
            .child(if snapshot.history_rows.is_empty() {
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
                    .children(
                        snapshot
                            .history_rows
                            .iter()
                            .enumerate()
                            .map(|(index, row)| self.clipboard_history_item(index, row, cx)),
                    )
                    .into_any_element()
            })
            .child(div().h_flex().justify_end().child(self.toolbar_button(
                "clipboard-history-load-more",
                match snapshot.history_load_state {
                    ClipboardHistoryLoadState::LoadingMore => "Loading",
                    _ => "Load More",
                },
                snapshot.can_load_more,
                theme::accent_amber(),
                |this, _, _, cx| this.load_more_clipboard_history(cx),
                cx,
            )))
    }

    fn clipboard_history_item(
        &self,
        index: usize,
        row: &ClipboardHistoryRowViewState,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let accent = self.clipboard_source_accent(row.record.source);
        let record = row.record.clone();

        div()
            .id(("clipboard-history-item", index))
            .cursor_pointer()
            .hover(|this| this.bg(theme::bg_panel_alt()))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.request_clipboard_select_history(record.event_id, window, cx);
            }))
            .child(
                clipboard_history_item_shell(row.selected, accent).child(
                    div()
                        .v_flex()
                        .gap(px(8.0))
                        .child(
                            div()
                                .h_flex()
                                .justify_between()
                                .gap(px(10.0))
                                .child(
                                    div()
                                        .text_size(px(12.0))
                                        .font_semibold()
                                        .text_color(theme::fg_primary())
                                        .child(row.record.origin_device_id.clone()),
                                )
                                .child(clipboard_badge(
                                    clipboard_source_label(row.record.source),
                                    accent,
                                )),
                        )
                        .child(
                            div()
                                .text_size(px(11.0))
                                .text_color(theme::fg_muted())
                                .child(format!(
                                    "{} · event {}",
                                    clipboard_record_time_label(&row.record),
                                    clipboard_short_event_id(&row.record)
                                )),
                        )
                        .child(
                            div()
                                .text_size(px(12.0))
                                .line_height(px(18.0))
                                .text_color(theme::fg_primary())
                                .child(clipboard_record_preview(&row.record.content, 92)),
                        ),
                ),
            )
    }
}
