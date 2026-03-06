use super::*;

impl WorkspaceView {
    fn radar_grid(&self) -> Div {
        let rings = [RADAR_MAX_RADIUS, 108.0, 68.0];

        div()
            .absolute()
            .top(px(0.0))
            .left(px(0.0))
            .right(px(0.0))
            .bottom(px(0.0))
            .child(
                div()
                    .absolute()
                    .top(px(0.0))
                    .left(px(0.0))
                    .right(px(0.0))
                    .bottom(px(0.0))
                    .bg(theme::accent_cyan().opacity(0.03)),
            )
            .children(rings.into_iter().map(|radius| {
                let size = radius * 2.0;
                div()
                    .absolute()
                    .top(px(RADAR_CENTER - radius))
                    .left(px(RADAR_CENTER - radius))
                    .size(px(size))
                    .rounded(px(999.0))
                    .border_1()
                    .border_color(theme::border_base().opacity(0.54))
            }))
            .child(
                div()
                    .absolute()
                    .top(px(RADAR_CENTER))
                    .left(px(30.0))
                    .right(px(30.0))
                    .h(px(1.0))
                    .bg(theme::border_soft().opacity(0.9)),
            )
            .child(
                div()
                    .absolute()
                    .top(px(30.0))
                    .bottom(px(30.0))
                    .left(px(RADAR_CENTER))
                    .w(px(1.0))
                    .bg(theme::border_soft().opacity(0.9)),
            )
            .child(
                div()
                    .absolute()
                    .top(px(58.0))
                    .left(px(58.0))
                    .right(px(58.0))
                    .bottom(px(58.0))
                    .rounded(px(999.0))
                    .border_1()
                    .border_color(theme::border_base().opacity(0.16)),
            )
    }

    fn radar_scan_layer(&self) -> Div {
        div()
            .absolute()
            .top(px(0.0))
            .left(px(0.0))
            .size(px(RADAR_SIZE))
            .child(
                svg()
                    .absolute()
                    .top(px(0.0))
                    .left(px(0.0))
                    .size(px(RADAR_SIZE))
                    .path(RADAR_SCAN_LINE_SVG)
                    .text_color(theme::accent_cyan().opacity(0.84))
                    .with_animation(
                        "system-core-scan-line",
                        Self::scan_animation(),
                        |this, delta| {
                            this.with_transformation(Transformation::rotate(percentage(delta)))
                        },
                    ),
            )
    }

    fn radar_core(&self) -> Div {
        div()
            .absolute()
            .top(px(RADAR_CENTER - 34.0))
            .left(px(RADAR_CENTER - 34.0))
            .size(px(68.0))
            .rounded(px(999.0))
            .bg(theme::accent_cyan().opacity(0.12))
            .border_1()
            .border_color(theme::accent_cyan().opacity(0.26))
            .shadow_xs()
            .flex()
            .items_center()
            .justify_center()
            .child(
                Icon::new(IconName::LayoutDashboard)
                    .size(px(20.0))
                    .text_color(theme::accent_cyan()),
            )
    }

    fn radar_peer_dot(
        &self,
        index: usize,
        peer: &SystemPeer,
        duplicate_device_ids: &HashMap<String, usize>,
    ) -> impl IntoElement {
        let visual_state = self.peer_visual_state(peer, duplicate_device_ids);
        let accent = Self::peer_state_accent(visual_state);
        let (x, y, phase) = Self::peer_position(peer);
        let dot_size = 12.0;
        let tooltip = match visual_state {
            RadarPeerVisualState::DeviceConflict => format!(
                "{}\n{}\nnode {}\nshared device_id",
                peer.ip,
                peer.device_id,
                Self::short_noob_id(&peer.noob_id)
            ),
            _ => format!(
                "{}\n{}\nnode {}",
                peer.ip,
                peer.device_id,
                Self::short_noob_id(&peer.noob_id)
            ),
        };

        div()
            .id(("system-core-peer-hotspot", index))
            .absolute()
            .left(px(x - dot_size / 2.0))
            .top(px(y - dot_size / 2.0))
            .size(px(dot_size))
            .cursor_pointer()
            .tooltip(move |window: &mut Window, cx| {
                Self::themed_tooltip(tooltip.clone(), window, cx)
            })
            .child(if self.network_service_enabled {
                div()
                    .size(px(dot_size))
                    .rounded(px(999.0))
                    .bg(accent.opacity(0.24))
                    .border_1()
                    .border_color(accent.opacity(0.34))
                    .shadow_xs()
                    .with_animation(
                        ("system-core-peer-dot", index),
                        Self::scan_animation(),
                        move |this, delta| {
                            let glow = Self::pulse_after_sweep(phase, delta);
                            this.bg(accent.opacity(0.24 + glow * 0.76))
                                .border_color(accent.opacity(0.34 + glow * 0.56))
                                .opacity(0.72 + glow * 0.28)
                        },
                    )
                    .into_any_element()
            } else {
                div()
                    .size(px(dot_size))
                    .rounded(px(999.0))
                    .bg(accent.opacity(0.12))
                    .border_1()
                    .border_color(accent.opacity(0.18))
                    .shadow_xs()
                    .into_any_element()
            })
    }

    pub(super) fn radar_panel(&self) -> Div {
        let duplicate_device_ids = self.duplicate_device_ids();

        div()
            .w(px(RADAR_PANEL_WIDTH))
            .h(px(RADAR_PANEL_HEIGHT))
            .bg(theme::bg_console())
            .border_1()
            .border_color(if self.network_service_enabled {
                theme::border_strong()
            } else {
                theme::border_soft()
            })
            .rounded(px(28.0))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .relative()
                    .size(px(RADAR_SIZE))
                    .rounded(px(999.0))
                    .overflow_hidden()
                    .child(self.radar_grid())
                    .when(self.network_service_enabled, |this| {
                        this.child(self.radar_scan_layer())
                    })
                    .child(self.radar_core())
                    .children(self.state.app.system_core.peers.iter().enumerate().map(
                        |(index, peer)| self.radar_peer_dot(index, peer, &duplicate_device_ids),
                    )),
            )
    }
}
