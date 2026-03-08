use gpui::{Div, Hsla, ParentElement, Styled, div, px};
use gpui_component::{Icon, IconName, StyledExt};

use crate::ui::theme;

pub(in crate::ui::workspace::view::peers) fn peers_panel_shell() -> Div {
    div()
        .bg(theme::bg_panel())
        .border_1()
        .border_color(theme::border_base())
        .shadow_xs()
}

pub(in crate::ui::workspace::view::peers) fn peers_panel_header(
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
                .text_size(px(11.0))
                .font_semibold()
                .text_color(theme::fg_muted())
                .child(detail.into()),
        )
}

pub(in crate::ui::workspace::view::peers) fn peers_summary_card(
    label: &'static str,
    value: usize,
    hint: &'static str,
    icon: IconName,
    accent: Hsla,
) -> Div {
    div()
        .flex_1()
        .min_w(px(0.0))
        .v_flex()
        .gap(px(10.0))
        .p(px(14.0))
        .bg(theme::bg_panel())
        .border_1()
        .border_color(theme::border_base())
        .rounded(px(20.0))
        .shadow_xs()
        .child(
            div()
                .h_flex()
                .items_center()
                .justify_between()
                .gap(px(10.0))
                .child(
                    div()
                        .h_flex()
                        .items_center()
                        .gap(px(8.0))
                        .child(
                            div()
                                .size(px(30.0))
                                .rounded(px(10.0))
                                .bg(accent.opacity(0.14))
                                .border_1()
                                .border_color(accent.opacity(0.28))
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(Icon::new(icon).size(px(14.0)).text_color(accent)),
                        )
                        .child(
                            div()
                                .text_size(px(11.0))
                                .font_semibold()
                                .text_color(theme::fg_secondary())
                                .child(label.to_string()),
                        ),
                )
                .child(div().size(px(6.0)).rounded(px(999.0)).bg(accent)),
        )
        .child(
            div()
                .text_size(px(30.0))
                .font_semibold()
                .text_color(theme::fg_primary())
                .child(value.to_string()),
        )
        .child(
            div()
                .text_size(px(11.0))
                .text_color(theme::fg_muted())
                .line_clamp(1)
                .text_ellipsis()
                .child(hint.to_string()),
        )
}

pub(in crate::ui::workspace::view::peers) fn peers_empty_state(filter_label: &str) -> Div {
    div()
        .w_full()
        .h(px(220.0))
        .v_flex()
        .items_center()
        .justify_center()
        .gap(px(12.0))
        .bg(theme::bg_console())
        .border_1()
        .border_color(theme::border_soft())
        .rounded(px(18.0))
        .child(
            div()
                .size(px(34.0))
                .rounded(px(12.0))
                .bg(theme::bg_panel_alt())
                .border_1()
                .border_color(theme::border_base())
                .flex()
                .items_center()
                .justify_center()
                .child(
                    Icon::new(IconName::Globe)
                        .size(px(16.0))
                        .text_color(theme::fg_muted()),
                ),
        )
        .child(
            div()
                .text_size(px(14.0))
                .font_semibold()
                .text_color(theme::fg_primary())
                .child("No peers match current filter"),
        )
        .child(
            div()
                .text_size(px(11.0))
                .text_color(theme::fg_muted())
                .child(format!("Filter: {}", filter_label)),
        )
}
