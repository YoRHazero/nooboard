use gpui::{Div, IntoElement, ParentElement, Styled, div, px};
use gpui_component::StyledExt;

use crate::ui::theme;

pub(super) fn clipboard_panel_shell() -> Div {
    div()
        .bg(theme::bg_panel())
        .border_1()
        .border_color(theme::border_soft())
        .rounded(px(24.0))
}

pub(super) fn clipboard_metric_chip(label: &str, value: String, accent: gpui::Hsla) -> Div {
    div()
        .h_flex()
        .items_center()
        .gap(px(8.0))
        .px(px(10.0))
        .py(px(8.0))
        .rounded(px(999.0))
        .bg(theme::bg_console())
        .border_1()
        .border_color(accent.opacity(0.24))
        .child(div().size(px(7.0)).rounded(px(999.0)).bg(accent))
        .child(
            div()
                .text_size(px(10.0))
                .font_semibold()
                .text_color(theme::fg_muted())
                .child(label.to_uppercase()),
        )
        .child(
            div()
                .text_size(px(12.0))
                .font_semibold()
                .text_color(theme::fg_primary())
                .child(value),
        )
}

pub(super) fn clipboard_badge(label: &str, accent: gpui::Hsla) -> Div {
    div()
        .h_flex()
        .items_center()
        .gap(px(6.0))
        .px(px(8.0))
        .py(px(4.0))
        .rounded(px(999.0))
        .bg(accent.opacity(0.12))
        .border_1()
        .border_color(accent.opacity(0.26))
        .child(div().size(px(6.0)).rounded(px(999.0)).bg(accent))
        .child(
            div()
                .text_size(px(10.0))
                .font_semibold()
                .text_color(theme::fg_primary())
                .child(label.to_string()),
        )
}

pub(super) fn clipboard_history_item_shell(selected: bool, accent: gpui::Hsla) -> Div {
    div()
        .w_full()
        .v_flex()
        .gap(px(8.0))
        .p(px(14.0))
        .rounded(px(18.0))
        .bg(if selected {
            theme::bg_panel_highlight()
        } else {
            theme::bg_console()
        })
        .border_1()
        .border_color(if selected {
            accent.opacity(0.32)
        } else {
            theme::border_soft()
        })
}

pub(super) fn clipboard_tab_chip(label: &str, selected: bool, accent: gpui::Hsla) -> Div {
    div()
        .px(px(12.0))
        .py(px(8.0))
        .rounded(px(999.0))
        .bg(if selected {
            accent.opacity(0.16)
        } else {
            theme::bg_console()
        })
        .border_1()
        .border_color(if selected {
            accent.opacity(0.30)
        } else {
            theme::border_soft()
        })
        .child(
            div()
                .text_size(px(12.0))
                .font_semibold()
                .text_color(if selected {
                    theme::fg_primary()
                } else {
                    theme::fg_secondary()
                })
                .child(label.to_string()),
        )
}

pub(super) fn clipboard_placeholder_copy(title: &str, body: &str) -> impl IntoElement {
    div()
        .v_flex()
        .items_center()
        .justify_center()
        .gap(px(10.0))
        .p(px(24.0))
        .child(
            div()
                .text_size(px(18.0))
                .font_semibold()
                .text_color(theme::fg_primary())
                .child(title.to_string()),
        )
        .child(
            div()
                .text_size(px(12.0))
                .text_color(theme::fg_muted())
                .child(body.to_string()),
        )
}
