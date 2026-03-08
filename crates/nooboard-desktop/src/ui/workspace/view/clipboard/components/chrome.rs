use gpui::{AnyView, App, Div, Hsla, ParentElement, Styled, Window, div, px};
use gpui_component::StyledExt;
use gpui_component::tooltip::Tooltip;

use crate::ui::theme;

pub(in crate::ui::workspace::view::clipboard) fn clipboard_panel_shell() -> Div {
    div()
        .bg(theme::bg_panel())
        .border_1()
        .border_color(theme::border_base())
        .shadow_xs()
}

pub(in crate::ui::workspace::view::clipboard) fn clipboard_panel_header(
    title: &str,
    detail: impl Into<String>,
) -> Div {
    div()
        .h_flex()
        .items_center()
        .justify_between()
        .gap(px(16.0))
        .child(
            div()
                .text_size(px(16.0))
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

pub(in crate::ui::workspace::view::clipboard) fn clipboard_badge(
    label: impl Into<String>,
    accent: Hsla,
) -> Div {
    div()
        .px(px(10.0))
        .py(px(6.0))
        .rounded(px(999.0))
        .bg(accent.opacity(0.14))
        .border_1()
        .border_color(accent.opacity(0.28))
        .text_size(px(10.0))
        .font_semibold()
        .text_color(accent)
        .child(label.into())
}

pub(in crate::ui::workspace::view::clipboard) fn clipboard_themed_tooltip(
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
