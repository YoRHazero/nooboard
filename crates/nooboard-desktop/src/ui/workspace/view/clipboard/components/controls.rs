use gpui::{
    AnyElement, App, Hsla, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, div, px,
};
use gpui_component::button::{Button, ButtonCustomVariant, ButtonVariants};
use gpui_component::{Disableable, StyledExt};

use crate::ui::theme;

use super::chrome::clipboard_themed_tooltip;

pub(in crate::ui::workspace::view::clipboard) fn clipboard_metric_chip(
    label: &str,
    value: String,
    accent: Hsla,
) -> gpui::Div {
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

pub(in crate::ui::workspace::view::clipboard) fn clipboard_action_button(
    id: impl Into<gpui::ElementId>,
    label: &str,
    accent: Hsla,
    disabled: bool,
    cx: &App,
) -> Button {
    let variant = ButtonCustomVariant::new(cx)
        .color(accent.opacity(0.10))
        .foreground(theme::fg_primary())
        .hover(accent.opacity(0.34))
        .active(accent.opacity(0.48))
        .shadow(false);

    Button::new(id)
        .custom(variant)
        .rounded(px(999.0))
        .border_1()
        .border_color(accent.opacity(0.38))
        .disabled(disabled)
        .child(
            div()
                .text_color(theme::fg_primary())
                .font_semibold()
                .child(label.to_string()),
        )
}

pub(in crate::ui::workspace::view::clipboard) fn clipboard_action_with_tooltip(
    id: &'static str,
    button: Button,
    tooltip: Option<String>,
) -> AnyElement {
    match tooltip {
        Some(text) => div()
            .id(id)
            .tooltip(move |window, cx| clipboard_themed_tooltip(text.clone(), window, cx))
            .child(button)
            .into_any_element(),
        None => button.into_any_element(),
    }
}
