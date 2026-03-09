mod clipboard;
mod components;
mod controls;
mod header;
mod radar;

use std::f32::consts::TAU;
use std::time::Duration;

use gpui::{
    Animation, AnimationExt as _, AnyView, App, Context, Div, Hsla, InteractiveElement,
    IntoElement, ParentElement, StatefulInteractiveElement, Styled, Transformation, Window, div,
    linear, percentage, prelude::FluentBuilder as _, px, svg,
};
use gpui_component::StyledExt;
use gpui_component::tooltip::Tooltip;

use crate::ui::theme;

use self::components::{
    arc_port_toggle_visual, clipboard_action_placeholder, clipboard_action_shell,
    clipboard_read_board, radar_panel_shell, system_core_card_shell, system_core_title_lockup,
};
use super::snapshot::HomeSystemCoreSnapshot;

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

impl WorkspaceView {
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

    pub(super) fn system_core_card(
        &self,
        snapshot: &HomeSystemCoreSnapshot,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        system_core_card_shell()
            .child(self.system_core_header(snapshot))
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
                            .child(self.radar_panel(snapshot))
                            .child(self.toggle_dock(snapshot, cx)),
                    )
                    .child(self.clipboard_panel(snapshot, cx)),
            )
            .with_animation("system-core-card", enter_animation(), |this, delta| {
                this.opacity(0.35 + delta * 0.65)
            })
    }
}
