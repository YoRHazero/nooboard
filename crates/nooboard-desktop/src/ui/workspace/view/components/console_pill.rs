use gpui::{Div, ParentElement, Styled, div, px};
use gpui_component::StyledExt;

use crate::ui::theme;

pub(crate) fn console_pill(label: &str, accent: gpui::Hsla) -> Div {
    div()
        .h_flex()
        .items_center()
        .gap(px(8.0))
        .px(px(10.0))
        .py(px(6.0))
        .bg(theme::bg_console())
        .border_1()
        .border_color(accent.opacity(0.24))
        .rounded(px(999.0))
        .child(div().size(px(6.0)).rounded(px(999.0)).bg(accent))
        .child(
            div()
                .text_size(px(10.0))
                .font_semibold()
                .text_color(theme::fg_secondary())
                .child(label.to_uppercase()),
        )
}
