use gpui::{Div, ParentElement, Styled, div, px};
use gpui_component::StyledExt;

use crate::ui::theme;

pub(crate) fn titlebar_chip(label: &str, value: String, accent: gpui::Hsla) -> Div {
    div()
        .h_flex()
        .h(px(22.0))
        .gap(px(8.0))
        .items_center()
        .px(px(8.0))
        .bg(theme::bg_panel_alt())
        .rounded(px(999.0))
        .child(div().size(px(6.0)).rounded(px(999.0)).bg(accent))
        .child(
            div()
                .text_size(px(10.0))
                .font_semibold()
                .text_color(theme::fg_secondary())
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
