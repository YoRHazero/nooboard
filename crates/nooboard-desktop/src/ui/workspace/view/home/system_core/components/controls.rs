use gpui::{Div, Hsla, ParentElement, Styled, div, px, svg};
use gpui_component::{Icon, IconName};

use crate::ui::theme;

const ARC_PORT_SOCKET_SVG: &str = "system_core/arc_port_socket.svg";
const ARC_PORT_NODE_SIZE: f32 = 74.0;

pub(in crate::ui::workspace::view::home::system_core) fn arc_port_toggle_visual(
    icon: IconName,
    active: bool,
    disabled: bool,
    accent: Hsla,
) -> Div {
    div()
        .relative()
        .size(px(ARC_PORT_NODE_SIZE))
        .child(
            div()
                .absolute()
                .top(px(11.0))
                .left(px(11.0))
                .size(px(52.0))
                .rounded(px(999.0))
                .bg(accent.opacity(if disabled {
                    0.03
                } else if active {
                    0.09
                } else {
                    0.02
                })),
        )
        .child(
            svg()
                .absolute()
                .top(px(0.0))
                .left(px(0.0))
                .size(px(ARC_PORT_NODE_SIZE))
                .path(ARC_PORT_SOCKET_SVG)
                .text_color(if disabled {
                    theme::border_soft().opacity(0.9)
                } else if active {
                    accent.opacity(0.92)
                } else {
                    theme::border_base().opacity(0.96)
                }),
        )
        .child(
            div()
                .absolute()
                .top(px(16.0))
                .left(px(16.0))
                .size(px(42.0))
                .rounded(px(999.0))
                .bg(if disabled {
                    theme::bg_panel_alt()
                } else if active {
                    accent.opacity(0.14)
                } else {
                    theme::bg_panel()
                })
                .border_1()
                .border_color(if disabled {
                    theme::border_soft().opacity(0.92)
                } else if active {
                    accent.opacity(0.32)
                } else {
                    theme::border_soft()
                }),
        )
        .child(
            div()
                .absolute()
                .top(px(16.0))
                .left(px(16.0))
                .size(px(42.0))
                .flex()
                .items_center()
                .justify_center()
                .child(Icon::new(icon).size(px(18.0)).text_color(if disabled {
                    theme::fg_muted().opacity(0.78)
                } else if active {
                    accent
                } else {
                    theme::fg_muted()
                })),
        )
        .child(
            div()
                .absolute()
                .left(px(32.0))
                .bottom(px(10.0))
                .size(px(10.0))
                .rounded(px(999.0))
                .bg(if disabled {
                    theme::border_soft()
                } else if active {
                    accent
                } else {
                    theme::border_base()
                }),
        )
}
