use super::super::snapshot::{
    HomeRadarPeerSnapshot, HomeRadarPeerVisualState, HomeRadarVisualState, HomeSystemCoreSnapshot,
};
use super::*;
use gpui::AnyElement;
use gpui_component::{Icon, IconName};

const RADAR_POWER_ICON_SVG: &str = "system_core/power.svg";

impl WorkspaceView {
    fn stable_unit(value: &str, salt: u64) -> f32 {
        let mut hash = 0xcbf29ce484222325u64 ^ salt;
        for byte in value.bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }

        (hash as f64 / u64::MAX as f64) as f32
    }

    fn short_noob_id(noob_id: &str) -> String {
        noob_id.chars().take(8).collect()
    }

    fn peer_position(noob_id: &str) -> (f32, f32, f32) {
        let angular = Self::stable_unit(noob_id, 0x1A11CE);
        let radial = Self::stable_unit(noob_id, 0xC0FFEE).sqrt();
        let theta = -std::f32::consts::FRAC_PI_2 + TAU * angular;
        let radius = RADAR_MIN_RADIUS + (RADAR_MAX_RADIUS - RADAR_MIN_RADIUS) * radial;
        let x = RADAR_CENTER + theta.cos() * radius;
        let y = RADAR_CENTER + theta.sin() * radius;
        (x, y, angular)
    }

    fn radar_border_color(state: HomeRadarVisualState) -> Hsla {
        match state {
            HomeRadarVisualState::Running | HomeRadarVisualState::Starting => {
                theme::border_strong()
            }
            HomeRadarVisualState::Stopped | HomeRadarVisualState::Disabled => theme::border_soft(),
            HomeRadarVisualState::Error => theme::accent_rose().opacity(0.92),
        }
    }

    fn radar_state_animation(state: HomeRadarVisualState) -> Option<Animation> {
        match state {
            HomeRadarVisualState::Starting => Some(
                Animation::new(Duration::from_secs_f64(4.0))
                    .repeat()
                    .with_easing(linear),
            ),
            HomeRadarVisualState::Error => Some(
                Animation::new(Duration::from_secs_f64(1.6))
                    .repeat()
                    .with_easing(linear),
            ),
            _ => None,
        }
    }

    fn radar_state_accent(state: HomeRadarVisualState) -> Hsla {
        match state {
            HomeRadarVisualState::Running | HomeRadarVisualState::Starting => theme::accent_cyan(),
            HomeRadarVisualState::Stopped => theme::accent_cyan().opacity(0.72),
            HomeRadarVisualState::Disabled => theme::accent_amber(),
            HomeRadarVisualState::Error => theme::accent_rose(),
        }
    }

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

    fn radar_core_icon(&self, state: HomeRadarVisualState) -> impl IntoElement {
        match state {
            HomeRadarVisualState::Disabled => svg()
                .w(px(22.0))
                .h(px(22.0))
                .path(RADAR_POWER_ICON_SVG)
                .text_color(theme::accent_amber())
                .into_any_element(),
            HomeRadarVisualState::Error => Icon::new(IconName::TriangleAlert)
                .size(px(22.0))
                .text_color(theme::accent_rose())
                .into_any_element(),
            _ => Icon::new(IconName::LayoutDashboard)
                .size(px(20.0))
                .text_color(theme::accent_cyan())
                .into_any_element(),
        }
    }

    fn radar_core(&self, state: HomeRadarVisualState) -> Div {
        let accent = Self::radar_state_accent(state);

        div()
            .absolute()
            .top(px(RADAR_CENTER - 34.0))
            .left(px(RADAR_CENTER - 34.0))
            .size(px(68.0))
            .rounded(px(999.0))
            .bg(accent.opacity(0.12))
            .border_1()
            .border_color(accent.opacity(0.26))
            .shadow_xs()
            .flex()
            .items_center()
            .justify_center()
            .child(self.radar_core_icon(state))
    }

    fn radar_peer_dot(
        &self,
        index: usize,
        peer: &HomeRadarPeerSnapshot,
        radar_state: HomeRadarVisualState,
    ) -> impl IntoElement {
        let accent = match peer.visual_state {
            HomeRadarPeerVisualState::Connected => theme::accent_green(),
            HomeRadarPeerVisualState::Transferring => theme::accent_blue(),
        };
        let (x, y, phase) = Self::peer_position(&peer.noob_id);
        let dot_size = 12.0;
        let tooltip = format!(
            "{}\ntransport {}\nnode {}",
            peer.address_label,
            peer.transport_label,
            Self::short_noob_id(&peer.noob_id)
        );

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
            .child(if radar_state.scans() {
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

    pub(super) fn radar_panel(&self, snapshot: &HomeSystemCoreSnapshot) -> impl IntoElement {
        let state = snapshot.radar.state;
        let panel = radar_panel_shell(Self::radar_border_color(state)).child(
            div()
                .relative()
                .size(px(RADAR_SIZE))
                .rounded(px(999.0))
                .overflow_hidden()
                .child(self.radar_grid())
                .when(snapshot.radar.state.scans(), |this| {
                    this.child(self.radar_scan_layer())
                })
                .child(self.radar_core(state))
                .children(
                    snapshot
                        .radar
                        .peers
                        .iter()
                        .enumerate()
                        .map(|(index, peer)| self.radar_peer_dot(index, peer, state)),
                ),
        );

        let panel: AnyElement = match Self::radar_state_animation(state) {
            Some(animation) if matches!(state, HomeRadarVisualState::Starting) => panel
                .with_animation("system-core-radar-starting", animation, |this, delta| {
                    let glow = 0.55 + 0.45 * (delta * TAU).sin().abs();
                    this.border_color(theme::accent_cyan().opacity(0.42 + glow * 0.44))
                })
                .into_any_element(),
            Some(animation) if matches!(state, HomeRadarVisualState::Error) => panel
                .with_animation("system-core-radar-error", animation, |this, delta| {
                    let flash = 0.45 + 0.55 * (delta * TAU).sin().abs();
                    this.border_color(theme::accent_rose().opacity(0.38 + flash * 0.54))
                })
                .into_any_element(),
            _ => panel.into_any_element(),
        };

        panel
    }
}
