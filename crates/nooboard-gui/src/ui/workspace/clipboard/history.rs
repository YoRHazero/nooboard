use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, div, px, uniform_list,
};
use gpui_component::StyledExt;

use crate::ui::theme;

use super::{
    CLIPBOARD_HISTORY_WIDTH, CLIPBOARD_PANEL_MIN_HEIGHT,
    components::{clipboard_badge, clipboard_history_item_shell, clipboard_panel_shell},
    view_state::{
        ClipboardHistoryGapViewState, ClipboardHistoryListItemViewState,
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
            .child(self.clipboard_history_window(snapshot, cx))
    }

    fn clipboard_history_window(
        &self,
        snapshot: &ClipboardPageViewState,
        cx: &Context<Self>,
    ) -> AnyElement {
        if snapshot.history_rows.is_empty() {
            return div()
                .w_full()
                .h(px(CLIPBOARD_PANEL_MIN_HEIGHT))
                .justify_center()
                .v_flex()
                .py(px(18.0))
                .text_size(px(11.0))
                .text_color(theme::fg_muted())
                .child("No earlier committed records are loaded yet.")
                .into_any_element();
        }

        let rows = snapshot.history_rows.clone();
        div()
            .w_full()
            .h(px(CLIPBOARD_PANEL_MIN_HEIGHT))
            .overflow_hidden()
            .child(
                uniform_list(
                    "clipboard-history-list",
                    rows.len(),
                    cx.processor(move |this, range: std::ops::Range<usize>, _, cx| {
                        range
                            .map(|index| match &rows[index] {
                                ClipboardHistoryListItemViewState::Record(row) => this
                                    .clipboard_history_item(index, row, cx)
                                    .into_any_element(),
                                ClipboardHistoryListItemViewState::Gap(gap) => this
                                    .clipboard_history_gap_item(index, gap, cx)
                                    .into_any_element(),
                            })
                            .collect::<Vec<AnyElement>>()
                    }),
                )
                .h_full(),
            )
            .into_any_element()
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

    fn clipboard_history_gap_item(
        &self,
        index: usize,
        gap: &ClipboardHistoryGapViewState,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let label = if gap.loading {
            match gap.direction {
                nooboard_core::ClipboardHistoryDirection::Older => "Loading older history…",
                nooboard_core::ClipboardHistoryDirection::Newer => "Loading newer history…",
            }
        } else {
            gap.label.as_str()
        }
        .to_string();

        let direction = gap.direction.clone();
        let interactive = gap.interactive && !gap.loading;

        let item = div()
            .id(("clipboard-history-gap", index))
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| {
                if !interactive {
                    return;
                }
                match direction {
                    nooboard_core::ClipboardHistoryDirection::Older => {
                        this.load_older_clipboard_history(cx)
                    }
                    nooboard_core::ClipboardHistoryDirection::Newer => {
                        this.load_newer_clipboard_history(cx)
                    }
                }
            }))
            .child(
                clipboard_history_item_shell(false, theme::accent_amber()).child(
                    div()
                        .v_flex()
                        .gap(px(4.0))
                        .child(
                            div()
                                .text_size(px(11.0))
                                .font_semibold()
                                .text_color(theme::fg_primary())
                                .child(label),
                        )
                        .child(
                            div()
                                .text_size(px(10.0))
                                .text_color(theme::fg_muted())
                                .child("Click to refill this unloaded history range."),
                        ),
                ),
            );

        if interactive {
            item.hover(|this| this.bg(theme::bg_panel_alt()))
        } else {
            item.cursor_default()
        }
    }
}
