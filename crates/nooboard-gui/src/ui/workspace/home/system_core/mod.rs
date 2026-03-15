mod clipboard;
mod components;
mod controls;
mod header;
mod radar;

use std::time::Duration;

use gpui::{
    Animation, AnimationExt as _, AnyView, App, Context, Hsla, IntoElement, ParentElement, Styled,
    Window, div, linear, percentage, px, svg,
};
use gpui_component::StyledExt;
use gpui_component::tooltip::Tooltip;

use crate::{
    ui::theme,
    workspace::view_state::{HomeRadarVisualState, HomeSystemCoreViewState},
};

use super::{WorkspaceView, enter_animation};
use components::system_core_card_shell;

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
        snapshot: &HomeSystemCoreViewState,
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

    fn radar_border_color(state: HomeRadarVisualState) -> Hsla {
        match state {
            HomeRadarVisualState::Running | HomeRadarVisualState::Starting => {
                theme::border_strong()
            }
            HomeRadarVisualState::Stopped => theme::border_soft(),
            HomeRadarVisualState::Error => theme::accent_rose().opacity(0.92),
        }
    }

    fn radar_state_accent(state: HomeRadarVisualState) -> Hsla {
        match state {
            HomeRadarVisualState::Running | HomeRadarVisualState::Starting => theme::accent_cyan(),
            HomeRadarVisualState::Stopped => theme::accent_cyan().opacity(0.72),
            HomeRadarVisualState::Error => theme::accent_rose(),
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

    fn radar_scan_layer(&self) -> gpui::Div {
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
                            this.with_transformation(gpui::Transformation::rotate(percentage(
                                delta,
                            )))
                        },
                    ),
            )
    }
}
