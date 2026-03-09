use gpui::{Div, Hsla, ParentElement, Styled, div, px};
use gpui_component::StyledExt;

use crate::ui::theme;

pub(in crate::ui::workspace::view::peers) fn peers_filter_chip(
    label: &str,
    count: usize,
    active: bool,
    accent: Hsla,
) -> Div {
    div()
        .h_flex()
        .items_center()
        .gap(px(8.0))
        .px(px(12.0))
        .py(px(8.0))
        .bg(if active {
            theme::bg_panel_highlight()
        } else {
            theme::bg_console()
        })
        .border_1()
        .border_color(if active {
            accent.opacity(0.34)
        } else {
            theme::border_soft()
        })
        .rounded(px(14.0))
        .child(
            div()
                .text_size(px(11.0))
                .font_semibold()
                .text_color(if active {
                    accent
                } else {
                    theme::fg_secondary()
                })
                .child(label.to_string()),
        )
        .child(
            div()
                .px(px(6.0))
                .py(px(2.0))
                .rounded(px(999.0))
                .bg(accent.opacity(if active { 0.2 } else { 0.12 }))
                .border_1()
                .border_color(accent.opacity(if active { 0.38 } else { 0.24 }))
                .text_size(px(10.0))
                .font_semibold()
                .text_color(accent)
                .child(count.to_string()),
        )
}

pub(in crate::ui::workspace::view::peers) fn peer_status_badge(label: &str, accent: Hsla) -> Div {
    div()
        .h_flex()
        .items_center()
        .gap(px(8.0))
        .px(px(10.0))
        .py(px(5.0))
        .bg(accent.opacity(0.13))
        .border_1()
        .border_color(accent.opacity(0.28))
        .rounded(px(999.0))
        .child(div().size(px(6.0)).rounded(px(999.0)).bg(accent))
        .child(
            div()
                .text_size(px(10.0))
                .font_semibold()
                .text_color(accent)
                .child(label.to_string()),
        )
}
