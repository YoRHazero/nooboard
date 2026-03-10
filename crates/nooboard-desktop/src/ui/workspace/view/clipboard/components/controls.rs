use gpui::{App, Hsla, ParentElement, Styled, div, px};
use gpui_component::Disableable;
use gpui_component::Sizable;
use gpui_component::button::{Button, ButtonCustomVariant, ButtonVariants};
use gpui_component::{Icon, IconName, StyledExt};

use crate::ui::theme;

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

pub(in crate::ui::workspace::view::clipboard) fn clipboard_icon_action_button(
    id: impl Into<gpui::ElementId>,
    icon: IconName,
    accent: Hsla,
    disabled: bool,
    cx: &App,
) -> Button {
    let variant = ButtonCustomVariant::new(cx)
        .color(accent.opacity(0.10))
        .foreground(theme::fg_primary())
        .hover(accent.opacity(0.22))
        .active(accent.opacity(0.30))
        .shadow(false);

    Button::new(id)
        .custom(variant)
        .small()
        .compact()
        .rounded(px(999.0))
        .border_1()
        .border_color(accent.opacity(0.28))
        .disabled(disabled)
        .icon(
            Icon::new(icon)
                .size(px(17.0))
                .text_color(theme::fg_primary()),
        )
}

pub(in crate::ui::workspace::view::clipboard) fn clipboard_mode_tab(
    label: &str,
    selected: bool,
    accent: Hsla,
) -> gpui::Div {
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
            accent.opacity(0.34)
        } else {
            theme::border_soft()
        })
        .child(
            div()
                .text_size(px(11.0))
                .font_semibold()
                .text_color(if selected {
                    accent
                } else {
                    theme::fg_secondary()
                })
                .child(label.to_string()),
        )
}
