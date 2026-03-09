use gpui::{Div, Hsla, Styled, div, px};

use crate::ui::theme;

pub(in crate::ui::workspace::view::home::system_core) fn radar_panel_shell(
    border_color: Hsla,
) -> Div {
    div()
        .w(px(super::super::RADAR_PANEL_WIDTH))
        .h(px(super::super::RADAR_PANEL_HEIGHT))
        .bg(theme::bg_console())
        .border_1()
        .border_color(border_color)
        .rounded(px(28.0))
        .flex()
        .items_center()
        .justify_center()
}
