use gpui::{Div, ParentElement, Styled, div, px};
use gpui_component::StyledExt;

use crate::ui::theme;

use super::console_pill;

pub(crate) fn command_bar(label: &str, detail: &str, accent: gpui::Hsla) -> Div {
    div()
        .h_flex()
        .items_center()
        .justify_between()
        .gap(px(16.0))
        .p(px(12.0))
        .bg(theme::bg_panel_alt())
        .border_1()
        .border_color(theme::border_soft())
        .rounded(px(18.0))
        .child(
            div()
                .h_flex()
                .items_center()
                .gap(px(10.0))
                .child(div().size(px(8.0)).rounded(px(999.0)).bg(accent))
                .child(
                    div()
                        .v_flex()
                        .gap(px(2.0))
                        .child(
                            div()
                                .text_size(px(10.0))
                                .font_semibold()
                                .text_color(accent)
                                .child(label.to_uppercase()),
                        )
                        .child(
                            div()
                                .text_size(px(11.0))
                                .text_color(theme::fg_muted())
                                .line_clamp(1)
                                .text_ellipsis()
                                .child(detail.to_string()),
                        ),
                ),
        )
        .child(
            div()
                .h_flex()
                .items_center()
                .gap(px(8.0))
                .child(console_pill("signal", accent))
                .child(console_pill("interpret", theme::accent_cyan())),
        )
}
