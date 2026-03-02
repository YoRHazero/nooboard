use gpui::{Div, ParentElement, Styled, div, px};
use gpui_component::{Icon, IconName, StyledExt};

use crate::ui::theme;

pub(crate) fn titlebar_brand() -> Div {
    div()
        .h_flex()
        .items_center()
        .gap(px(10.0))
        .child(
            div()
                .size(px(22.0))
                .rounded(px(8.0))
                .bg(theme::bg_panel_alt())
                .flex()
                .items_center()
                .justify_center()
                .child(
                    Icon::new(IconName::LayoutDashboard)
                        .size(px(12.0))
                        .text_color(theme::accent_cyan()),
                ),
        )
        .child(
            div()
                .text_size(px(13.0))
                .font_semibold()
                .text_color(theme::fg_primary())
                .child("Nooboard Control"),
        )
        .child(
            div()
                .text_size(px(11.0))
                .text_color(theme::fg_secondary())
                .child("mesh desktop"),
        )
}
