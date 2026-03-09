use gpui::{App, Div, Hsla, ParentElement, Styled, div, px};
use gpui_component::Sizable;
use gpui_component::StyledExt;
use gpui_component::button::{Button, ButtonCustomVariant, ButtonVariants};

use crate::ui::theme;

pub(in crate::ui::workspace::view::transfers) fn transfer_metric_chip(
    label: &str,
    value: String,
    accent: Hsla,
) -> Div {
    div()
        .h_flex()
        .items_center()
        .gap(px(8.0))
        .px(px(12.0))
        .py(px(9.0))
        .bg(theme::bg_console())
        .border_1()
        .border_color(accent.opacity(0.22))
        .rounded(px(16.0))
        .child(
            div()
                .text_size(px(10.0))
                .font_semibold()
                .text_color(accent)
                .child(label.to_string()),
        )
        .child(
            div()
                .text_size(px(13.0))
                .font_semibold()
                .text_color(theme::fg_primary())
                .child(value),
        )
}

pub(in crate::ui::workspace::view::transfers) fn transfer_action_button(
    id: impl Into<gpui::ElementId>,
    label: &str,
    accent: Hsla,
    cx: &App,
) -> Button {
    let variant = ButtonCustomVariant::new(cx)
        .color(accent.opacity(0.12))
        .foreground(theme::fg_primary())
        .hover(accent.opacity(0.2))
        .active(accent.opacity(0.28))
        .shadow(false);

    Button::new(id)
        .custom(variant)
        .small()
        .compact()
        .rounded(px(999.0))
        .border_1()
        .border_color(accent.opacity(0.24))
        .child(
            div()
                .text_color(theme::fg_primary())
                .font_semibold()
                .child(label.to_string()),
        )
}
