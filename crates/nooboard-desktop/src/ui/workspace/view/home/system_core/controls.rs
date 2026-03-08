use super::*;

const ARC_PORT_TRACK_SVG: &str = "system_core/arc_port_track.svg";
const ARC_PORT_SIGNAL_SVG: &str = "system_core/arc_port_signal.svg";
const ARC_PORT_WIDTH: f32 = 316.0;
const ARC_PORT_HEIGHT: f32 = 116.0;
const ARC_PORT_NODE_SIZE: f32 = 74.0;

impl WorkspaceView {
    fn arc_port_toggle(
        &self,
        cx: &mut Context<Self>,
        id: &'static str,
        left: f32,
        icon: IconName,
        active: bool,
        accent: Hsla,
        tooltip_title: &str,
        tooltip_detail: &str,
    ) -> impl IntoElement {
        let tooltip = format!("{}\n{}", tooltip_title, tooltip_detail);

        div()
            .id(match id {
                "bridge" => "system-core-arc-port-bridge",
                _ => "system-core-arc-port-network",
            })
            .absolute()
            .top(px(6.0))
            .left(px(left))
            .size(px(ARC_PORT_NODE_SIZE))
            .cursor_pointer()
            .tooltip(move |window: &mut Window, cx| {
                Self::themed_tooltip(tooltip.clone(), window, cx)
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                match id {
                    "bridge" => this.auto_bridge_remote_text = !this.auto_bridge_remote_text,
                    "network" => this.network_service_enabled = !this.network_service_enabled,
                    _ => {}
                }
                cx.notify();
            }))
            .child(arc_port_toggle_visual(icon, active, accent))
    }

    pub(super) fn toggle_dock(&self, cx: &mut Context<Self>) -> Div {
        div().w_full().flex().justify_center().child(
            div()
                .relative()
                .w(px(ARC_PORT_WIDTH))
                .h(px(ARC_PORT_HEIGHT))
                .child(
                    div()
                        .absolute()
                        .left(px(62.0))
                        .right(px(62.0))
                        .bottom(px(14.0))
                        .h(px(30.0))
                        .bg(theme::bg_console())
                        .border_1()
                        .border_color(theme::border_soft().opacity(0.88))
                        .rounded(px(999.0)),
                )
                .child(
                    svg()
                        .absolute()
                        .top(px(10.0))
                        .left(px(6.0))
                        .w(px(304.0))
                        .h(px(96.0))
                        .path(ARC_PORT_TRACK_SVG)
                        .text_color(theme::border_base().opacity(0.84)),
                )
                .child(
                    svg()
                        .absolute()
                        .top(px(67.0))
                        .left(px(38.0))
                        .w(px(76.0))
                        .h(px(24.0))
                        .path(ARC_PORT_SIGNAL_SVG)
                        .text_color(if self.network_service_enabled {
                            theme::accent_cyan()
                        } else {
                            theme::border_base().opacity(0.92)
                        }),
                )
                .child(
                    svg()
                        .absolute()
                        .top(px(67.0))
                        .left(px(202.0))
                        .w(px(76.0))
                        .h(px(24.0))
                        .path(ARC_PORT_SIGNAL_SVG)
                        .text_color(if self.auto_bridge_remote_text {
                            theme::accent_blue()
                        } else {
                            theme::border_base().opacity(0.92)
                        }),
                )
                .child(
                    div()
                        .absolute()
                        .left(px(153.0))
                        .bottom(px(16.0))
                        .size(px(6.0))
                        .rounded(px(999.0))
                        .bg(theme::border_base().opacity(0.92)),
                )
                .child(self.arc_port_toggle(
                    cx,
                    "network",
                    39.0,
                    IconName::Globe,
                    self.network_service_enabled,
                    theme::accent_cyan(),
                    "Network service",
                    if self.network_service_enabled {
                        "radar scan active"
                    } else {
                        "radar scan halted"
                    },
                ))
                .child(self.arc_port_toggle(
                    cx,
                    "bridge",
                    203.0,
                    IconName::Copy,
                    self.auto_bridge_remote_text,
                    theme::accent_blue(),
                    "Remote relay",
                    if self.auto_bridge_remote_text {
                        "auto bridge to clipboard and storage"
                    } else {
                        "manual adopt required"
                    },
                )),
        )
    }
}
