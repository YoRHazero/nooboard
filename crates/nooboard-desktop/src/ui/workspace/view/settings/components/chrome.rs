use gpui::{AnyElement, Div, Hsla, ParentElement, Styled, div, px};
use gpui_component::StyledExt;

use crate::ui::theme;

pub(in crate::ui::workspace::view::settings) fn settings_feedback_banner(
    label: &str,
    accent: Hsla,
    message: impl Into<String>,
) -> Div {
    div()
        .min_h(px(52.0))
        .h_flex()
        .items_center()
        .justify_between()
        .gap(px(14.0))
        .px(px(16.0))
        .py(px(12.0))
        .bg(theme::bg_panel())
        .border_1()
        .border_color(accent.opacity(0.28))
        .rounded(px(16.0))
        .child(settings_status_chip(label, accent))
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .text_size(px(12.0))
                .text_color(theme::fg_secondary())
                .line_clamp(2)
                .text_ellipsis()
                .child(message.into()),
        )
}

pub(in crate::ui::workspace::view::settings) fn settings_status_chip(
    label: impl Into<String>,
    accent: Hsla,
) -> Div {
    div()
        .h_flex()
        .items_center()
        .justify_center()
        .min_w(px(96.0))
        .gap(px(8.0))
        .px(px(10.0))
        .py(px(6.0))
        .bg(accent.opacity(0.12))
        .border_1()
        .border_color(accent.opacity(0.26))
        .rounded(px(999.0))
        .child(div().size(px(6.0)).rounded(px(999.0)).bg(accent))
        .child(
            div()
                .text_size(px(10.0))
                .font_semibold()
                .text_center()
                .text_color(accent)
                .child(label.into()),
        )
}

pub(in crate::ui::workspace::view::settings) fn settings_section_shell(
    title: &str,
    description: &str,
    status: Div,
) -> Div {
    div()
        .w_full()
        .v_flex()
        .gap(px(14.0))
        .p(px(18.0))
        .bg(theme::bg_panel())
        .border_1()
        .border_color(theme::border_base())
        .rounded(px(22.0))
        .shadow_xs()
        .child(settings_section_header(title, description, status))
        .child(div().h(px(1.0)).w_full().bg(theme::border_soft()))
}

fn settings_section_header(title: &str, description: &str, status: Div) -> Div {
    div()
        .h_flex()
        .items_start()
        .justify_between()
        .gap(px(12.0))
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .v_flex()
                .gap(px(6.0))
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
                        .line_clamp(2)
                        .text_ellipsis()
                        .child(description.to_string()),
                ),
        )
        .child(status)
}

pub(in crate::ui::workspace::view::settings) fn settings_section_footer(
    summary_text: impl Into<String>,
    summary_color: Hsla,
    actions: Div,
) -> Div {
    div()
        .pt(px(8.0))
        .v_flex()
        .gap(px(10.0))
        .child(
            div()
                .min_h(px(30.0))
                .text_size(px(11.0))
                .text_color(summary_color)
                .line_clamp(2)
                .text_ellipsis()
                .child(summary_text.into()),
        )
        .child(actions)
}

pub(in crate::ui::workspace::view::settings) fn settings_action_row(
    actions: impl IntoIterator<Item = AnyElement>,
) -> Div {
    div().h_flex().justify_end().gap(px(8.0)).children(actions)
}
