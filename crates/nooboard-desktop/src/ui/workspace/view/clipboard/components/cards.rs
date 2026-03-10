use gpui::{Div, Hsla, ParentElement, Styled, div, px};
use gpui_component::StyledExt;

use crate::ui::theme;

pub(in crate::ui::workspace::view::clipboard) fn clipboard_target_chip(
    device_id: String,
    selected: bool,
    interactive: bool,
    accent: Hsla,
) -> Div {
    div()
        .min_w(px(152.0))
        .px(px(14.0))
        .py(px(12.0))
        .rounded(px(18.0))
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
        .opacity(if interactive { 1.0 } else { 0.74 })
        .child(
            div()
                .h_flex()
                .items_center()
                .justify_between()
                .gap(px(12.0))
                .child(
                    div()
                        .text_size(px(12.0))
                        .font_semibold()
                        .text_color(theme::fg_primary())
                        .child(device_id),
                )
                .child(
                    div()
                        .h_flex()
                        .items_center()
                        .gap(px(6.0))
                        .child(div().size(px(6.0)).rounded(px(999.0)).bg(accent))
                        .child(
                            div()
                                .text_size(px(10.0))
                                .font_semibold()
                                .text_color(if interactive {
                                    accent
                                } else {
                                    theme::fg_muted()
                                })
                                .child(if interactive { "Selected" } else { "Broadcast" }),
                        ),
                ),
        )
}

pub(in crate::ui::workspace::view::clipboard) fn clipboard_history_item_shell(
    selected: bool,
    accent: Hsla,
) -> Div {
    div()
        .w_full()
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
}

pub(in crate::ui::workspace::view::clipboard) fn clipboard_history_item_body(
    title: String,
    recorded_at_label: String,
    source_badge: Div,
    preview: String,
) -> Div {
    div()
        .w_full()
        .v_flex()
        .gap(px(10.0))
        .child(
            div()
                .w_full()
                .h_flex()
                .items_start()
                .gap(px(12.0))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .v_flex()
                        .gap(px(5.0))
                        .child(
                            div()
                                .w_full()
                                .text_size(px(12.0))
                                .font_semibold()
                                .text_color(theme::fg_primary())
                                .line_clamp(1)
                                .text_ellipsis()
                                .child(title),
                        )
                        .child(
                            div()
                                .w_full()
                                .text_size(px(10.0))
                                .font_semibold()
                                .text_color(theme::fg_muted())
                                .line_clamp(1)
                                .text_ellipsis()
                                .child(recorded_at_label),
                        ),
                )
                .child(div().flex_shrink_0().child(source_badge)),
        )
        .child(
            div()
                .w_full()
                .text_size(px(12.0))
                .text_color(theme::fg_secondary())
                .line_clamp(2)
                .text_ellipsis()
                .child(preview),
        )
}
