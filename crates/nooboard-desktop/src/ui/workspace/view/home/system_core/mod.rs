mod clipboard;
mod controls;
mod header;
mod radar;

use std::time::Duration;
use std::{collections::HashMap, f32::consts::TAU};

use gpui::{
    Animation, AnimationExt as _, AnyView, App, Context, Div, Hsla, InteractiveElement,
    IntoElement, ParentElement, StatefulInteractiveElement, Styled, Transformation, Window, div,
    linear, percentage, prelude::FluentBuilder as _, px, svg,
};
use gpui_component::clipboard::Clipboard;
use gpui_component::tooltip::Tooltip;
use gpui_component::{Icon, IconName, StyledExt};

use crate::{
    state::{ClipboardTextItem, ClipboardTextOrigin, SystemPeer, SystemPeerStatus},
    ui::theme,
};

use super::super::{WorkspaceView, shared::enter_animation};

const RADAR_SIZE: f32 = 404.0;
const RADAR_CENTER: f32 = RADAR_SIZE / 2.0;
const RADAR_MIN_RADIUS: f32 = 52.0;
const RADAR_MAX_RADIUS: f32 = 150.0;
const RADAR_PANEL_WIDTH: f32 = 422.0;
const RADAR_PANEL_HEIGHT: f32 = 456.0;
const CLIPBOARD_PANEL_WIDTH: f32 = 276.0;
const CLIPBOARD_PANEL_HEIGHT: f32 = 492.0;
const RADAR_SCAN_LINE_SVG: &str = "system_core/radar_scan_line.svg";

#[derive(Clone, Copy, PartialEq, Eq)]
enum RadarPeerVisualState {
    Connected,
    Transferring,
    DeviceConflict,
}

impl WorkspaceView {
    fn short_noob_id(noob_id: &str) -> String {
        noob_id.chars().take(8).collect()
    }

    fn stable_unit(value: &str, salt: u64) -> f32 {
        let mut hash = 0xcbf29ce484222325u64 ^ salt;
        for byte in value.bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }

        (hash as f64 / u64::MAX as f64) as f32
    }

    fn duplicate_device_ids(&self) -> HashMap<String, usize> {
        let mut counts = HashMap::new();
        for peer in &self.state.app.system_core.peers {
            *counts.entry(peer.device_id.clone()).or_insert(0) += 1;
        }
        counts
    }

    fn peer_visual_state(
        &self,
        peer: &SystemPeer,
        duplicate_device_ids: &HashMap<String, usize>,
    ) -> RadarPeerVisualState {
        if duplicate_device_ids
            .get(peer.device_id.as_str())
            .copied()
            .unwrap_or_default()
            > 1
        {
            return RadarPeerVisualState::DeviceConflict;
        }

        match peer.status {
            SystemPeerStatus::Connected => RadarPeerVisualState::Connected,
            SystemPeerStatus::Transferring => RadarPeerVisualState::Transferring,
        }
    }

    fn peer_state_accent(state: RadarPeerVisualState) -> Hsla {
        match state {
            RadarPeerVisualState::Connected => theme::accent_green(),
            RadarPeerVisualState::Transferring => theme::accent_blue(),
            RadarPeerVisualState::DeviceConflict => theme::accent_amber(),
        }
    }

    fn peer_position(peer: &SystemPeer) -> (f32, f32, f32) {
        let angular = Self::stable_unit(&peer.noob_id, 0x1A11CE);
        let radial = Self::stable_unit(&peer.noob_id, 0xC0FFEE).sqrt();
        let theta = -std::f32::consts::FRAC_PI_2 + TAU * angular;
        let radius = RADAR_MIN_RADIUS + (RADAR_MAX_RADIUS - RADAR_MIN_RADIUS) * radial;
        let x = RADAR_CENTER + theta.cos() * radius;
        let y = RADAR_CENTER + theta.sin() * radius;
        (x, y, angular)
    }

    fn pulse_after_sweep(phase: f32, delta: f32) -> f32 {
        let trail = (delta - phase).rem_euclid(1.0);

        if trail < 0.14 {
            1.0 - trail / 0.14
        } else {
            0.0
        }
    }

    fn scan_animation() -> Animation {
        Animation::new(Duration::from_secs_f64(4.8))
            .repeat()
            .with_easing(linear)
    }

    fn themed_tooltip(text: String, window: &mut Window, cx: &mut App) -> AnyView {
        Tooltip::new(text)
            .bg(theme::bg_panel())
            .text_color(theme::fg_primary())
            .border_color(theme::border_base())
            .build(window, cx)
    }

    pub(super) fn system_core_card(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .v_flex()
            .gap(px(18.0))
            .p(px(22.0))
            .bg(theme::bg_panel())
            .border_1()
            .border_color(theme::border_base())
            .rounded(px(28.0))
            .shadow_xs()
            .child(self.system_core_header())
            .child(
                div()
                    .h_flex()
                    .items_start()
                    .justify_center()
                    .gap(px(16.0))
                    .child(
                        div()
                            .w(px(RADAR_PANEL_WIDTH))
                            .flex_shrink_0()
                            .v_flex()
                            .gap(px(12.0))
                            .child(self.radar_panel())
                            .child(self.toggle_dock(cx)),
                    )
                    .child(self.clipboard_panel()),
            )
            .with_animation("system-core-card", enter_animation(), |this, delta| {
                this.opacity(0.35 + delta * 0.65)
            })
    }
}
