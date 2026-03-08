use gpui::{Div, ParentElement, Styled, div, px};
use gpui_component::StyledExt;

use crate::ui::theme;

pub(in crate::ui::workspace::view::transfers) fn transfers_panel_shell() -> Div {
    div()
        .v_flex()
        .gap(px(14.0))
        .p(px(18.0))
        .bg(theme::bg_panel())
        .border_1()
        .border_color(theme::border_base())
        .rounded(px(24.0))
        .shadow_xs()
}

pub(in crate::ui::workspace::view::transfers) fn transfers_panel_header(
    title: &str,
    detail: impl Into<String>,
) -> Div {
    div()
        .h_flex()
        .items_center()
        .justify_between()
        .gap(px(12.0))
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
                .child(detail.into()),
        )
}

pub(in crate::ui::workspace::view::transfers) fn transfers_empty_notice(label: &str) -> Div {
    div()
        .p(px(12.0))
        .bg(theme::bg_activity())
        .border_1()
        .border_color(theme::border_soft())
        .rounded(px(16.0))
        .text_size(px(12.0))
        .text_color(theme::fg_muted())
        .child(label.to_string())
}

pub(in crate::ui::workspace::view::transfers) fn transfers_section(
    title: &str,
    count: usize,
    cards: Vec<Div>,
    empty_label: &str,
) -> Div {
    div()
        .v_flex()
        .gap(px(10.0))
        .child(
            div()
                .h_flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(px(14.0))
                        .font_semibold()
                        .text_color(theme::fg_primary())
                        .child(title.to_string()),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(theme::fg_muted())
                        .child(count.to_string()),
                ),
        )
        .children(if cards.is_empty() {
            vec![transfers_empty_notice(empty_label)]
        } else {
            cards
        })
}

pub(in crate::ui::workspace::view::transfers) fn transfers_card_shell() -> Div {
    div()
        .v_flex()
        .gap(px(10.0))
        .p(px(14.0))
        .bg(theme::bg_rail_panel())
        .border_1()
        .border_color(theme::border_soft())
        .rounded(px(18.0))
}
