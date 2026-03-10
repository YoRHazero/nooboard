use gpui::{AnyView, App, Hsla, ParentElement, Styled, Window, div, px};
use gpui_component::Sizable;
use gpui_component::StyledExt;
use gpui_component::button::{Button, ButtonCustomVariant, ButtonVariants};
use gpui_component::tooltip::Tooltip;

use crate::ui::theme;

pub(in crate::ui::workspace::view::settings) fn settings_themed_tooltip(
    text: String,
    window: &mut Window,
    cx: &mut App,
) -> AnyView {
    Tooltip::new(text)
        .bg(theme::bg_panel())
        .text_color(theme::fg_primary())
        .border_color(theme::border_base())
        .build(window, cx)
}

pub(in crate::ui::workspace::view::settings) fn settings_action_button(
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
        .min_w(px(82.0))
        .rounded(px(12.0))
        .border_1()
        .border_color(accent.opacity(0.24))
        .child(
            div()
                .w_full()
                .text_center()
                .text_color(theme::fg_primary())
                .font_semibold()
                .child(label.to_string()),
        )
}

pub(in crate::ui::workspace::view::settings) fn settings_control_button(
    id: impl Into<gpui::ElementId>,
    label: &str,
    accent: Hsla,
    cx: &App,
) -> Button {
    let variant = ButtonCustomVariant::new(cx)
        .color(accent.opacity(0.10))
        .foreground(theme::fg_primary())
        .hover(accent.opacity(0.18))
        .active(accent.opacity(0.24))
        .shadow(false);

    Button::new(id)
        .custom(variant)
        .small()
        .compact()
        .w(px(34.0))
        .rounded(px(12.0))
        .border_1()
        .border_color(accent.opacity(0.24))
        .child(
            div()
                .w_full()
                .text_center()
                .text_color(theme::fg_primary())
                .font_semibold()
                .child(label.to_string()),
        )
}
