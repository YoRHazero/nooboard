use gpui::{Div, Hsla, ParentElement, Styled, div, px};
use gpui_component::{Icon, IconName, StyledExt};

use crate::ui::theme;

pub(in crate::ui::workspace::view::home::system_core) fn system_core_card_shell() -> Div {
    div()
        .v_flex()
        .gap(px(18.0))
        .p(px(22.0))
        .bg(theme::bg_panel())
        .border_1()
        .border_color(theme::border_base())
        .rounded(px(28.0))
        .shadow_xs()
}

pub(in crate::ui::workspace::view::home::system_core) fn system_core_title_lockup(
    accent: Hsla,
) -> Div {
    div()
        .h_flex()
        .items_center()
        .gap(px(14.0))
        .child(
            div()
                .size(px(36.0))
                .rounded(px(12.0))
                .bg(accent.opacity(0.12))
                .border_1()
                .border_color(accent.opacity(0.24))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    Icon::new(IconName::LayoutDashboard)
                        .size(px(17.0))
                        .text_color(accent),
                ),
        )
        .child(
            div()
                .text_size(px(24.0))
                .font_semibold()
                .text_color(theme::fg_primary())
                .child("System Core"),
        )
}
