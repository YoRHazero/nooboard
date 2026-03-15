use gpui::{AnyElement, Div, Hsla, ParentElement, Styled, div, px, svg};
use gpui_component::{Icon, IconName, StyledExt};

use crate::ui::theme;

const ARC_PORT_SOCKET_SVG: &str = "system_core/arc_port_socket.svg";
const ARC_PORT_NODE_SIZE: f32 = 74.0;

pub(super) fn system_core_card_shell() -> Div {
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

pub(super) fn system_core_title_lockup(accent: Hsla) -> Div {
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

pub(super) fn radar_panel_shell(border_color: Hsla) -> Div {
    div()
        .w(px(super::RADAR_PANEL_WIDTH))
        .h(px(super::RADAR_PANEL_HEIGHT))
        .bg(theme::bg_console())
        .border_1()
        .border_color(border_color)
        .rounded(px(28.0))
        .flex()
        .items_center()
        .justify_center()
}

pub(super) fn arc_port_toggle_visual(
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

pub(super) fn clipboard_action_shell(accent: Hsla) -> Div {
    div()
        .size(px(34.0))
        .cursor_pointer()
        .rounded(px(12.0))
        .bg(theme::bg_panel_alt())
        .border_1()
        .border_color(accent.opacity(0.22))
        .flex()
        .items_center()
        .justify_center()
}

pub(super) fn clipboard_action_placeholder(accent: Hsla) -> Div {
    div()
        .size(px(34.0))
        .rounded(px(12.0))
        .bg(theme::bg_panel_alt())
        .border_1()
        .border_color(theme::border_soft())
        .flex()
        .items_center()
        .justify_center()
        .opacity(0.56)
        .child(
            Icon::new(IconName::Copy)
                .size(px(15.0))
                .text_color(accent.opacity(0.9)),
        )
}

pub(super) fn clipboard_read_board(
    device_id: String,
    recorded_at_label: String,
    accent: Hsla,
    action: AnyElement,
    content: String,
) -> Div {
    div()
        .relative()
        .v_flex()
        .size_full()
        .overflow_hidden()
        .child(
            div()
                .v_flex()
                .size_full()
                .gap(px(16.0))
                .p(px(18.0))
                .child(
                    div()
                        .h_flex()
                        .justify_between()
                        .items_start()
                        .gap(px(12.0))
                        .child(
                            div()
                                .v_flex()
                                .gap(px(6.0))
                                .child(
                                    div()
                                        .h_flex()
                                        .items_center()
                                        .gap(px(10.0))
                                        .child(div().size(px(8.0)).rounded(px(999.0)).bg(accent))
                                        .child(
                                            div()
                                                .text_size(px(13.0))
                                                .font_semibold()
                                                .text_color(theme::fg_primary())
                                                .truncate()
                                                .child(device_id),
                                        ),
                                )
                                .child(
                                    div()
                                        .text_size(px(10.0))
                                        .font_semibold()
                                        .text_color(theme::fg_muted())
                                        .child(recorded_at_label),
                                ),
                        )
                        .child(action),
                )
                .child(
                    div()
                        .w_full()
                        .h(px(1.0))
                        .bg(theme::border_soft().opacity(0.94)),
                )
                .child(
                    div().relative().flex_1().min_h(px(0.0)).child(
                        div()
                            .absolute()
                            .top(px(0.0))
                            .left(px(0.0))
                            .right(px(0.0))
                            .bottom(px(0.0))
                            .text_size(px(14.0))
                            .text_color(theme::fg_primary())
                            .line_clamp(12)
                            .text_ellipsis()
                            .child(content),
                    ),
                ),
        )
}
