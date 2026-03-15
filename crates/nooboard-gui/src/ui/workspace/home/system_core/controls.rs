use gpui::{
    Context, Hsla, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement,
    Styled, Window, div, prelude::FluentBuilder as _, px, svg,
};
use gpui_component::IconName;

use crate::{
    ui::theme,
    workspace::{
        actions::{clipboard as clipboard_actions, network as network_actions},
        view_state::HomeSystemCoreViewState,
    },
};

use super::{WorkspaceView, components::arc_port_toggle_visual};

const ARC_PORT_TRACK_SVG: &str = "system_core/arc_port_track.svg";
const ARC_PORT_SIGNAL_SVG: &str = "system_core/arc_port_signal.svg";
const ARC_PORT_WIDTH: f32 = 316.0;
const ARC_PORT_HEIGHT: f32 = 116.0;

#[derive(Clone, Copy)]
enum DockAction {
    Network,
    AdoptLatestClipboard(nooboard_core::EventId),
}

impl WorkspaceView {
    fn arc_port_toggle(
        &self,
        cx: &mut Context<Self>,
        id: &'static str,
        left: f32,
        icon: IconName,
        active: bool,
        disabled: bool,
        accent: Hsla,
        tooltip_title: &str,
        tooltip_detail: &str,
        action: Option<DockAction>,
    ) -> impl IntoElement {
        let tooltip = format!("{}\n{}", tooltip_title, tooltip_detail);

        div()
            .id(match id {
                "network" => "system-core-arc-port-network",
                _ => "system-core-arc-port-clipboard",
            })
            .absolute()
            .top(px(6.0))
            .left(px(left))
            .size(px(74.0))
            .when(!disabled, |this| this.cursor_pointer())
            .tooltip(move |window: &mut Window, cx| {
                Self::themed_tooltip(tooltip.clone(), window, cx)
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                if disabled {
                    return;
                }

                match action {
                    Some(DockAction::Network) => {
                        if active {
                            network_actions::stop_network(&this.controller, cx);
                        } else {
                            network_actions::start_network(&this.controller, cx);
                        }
                    }
                    Some(DockAction::AdoptLatestClipboard(event_id)) => {
                        if let Some(task) = clipboard_actions::adopt_clipboard_record_task(
                            &this.controller,
                            event_id,
                            cx,
                        ) {
                            task.detach();
                        }
                    }
                    None => {}
                }
            }))
            .child(arc_port_toggle_visual(icon, active, disabled, accent))
    }

    pub(super) fn toggle_dock(
        &self,
        snapshot: &HomeSystemCoreViewState,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let network_tooltip = if snapshot.network_control.can_stop {
            ("Stop network service", "pause sync discovery and transfers")
        } else if snapshot.network_control.can_start {
            (
                "Start network service",
                "enable sync discovery and transfers",
            )
        } else {
            (
                "Network action unavailable",
                "wait for the workspace runtime to settle",
            )
        };
        let clipboard_tooltip = if snapshot.clipboard_control.adopt_event_id.is_some() {
            (
                "Adopt latest clipboard",
                "write the latest committed text into the local clipboard",
            )
        } else {
            (
                "No committed clipboard",
                "wait for the first committed clipboard record to arrive",
            )
        };
        let network_active = snapshot.network_control.can_stop;
        let network_disabled =
            !snapshot.network_control.can_start && !snapshot.network_control.can_stop;
        let clipboard_active = snapshot.clipboard_control.adopt_event_id.is_some();

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
                        .text_color(if network_active {
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
                        .text_color(if clipboard_active {
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
                    network_active,
                    network_disabled,
                    theme::accent_cyan(),
                    network_tooltip.0,
                    network_tooltip.1,
                    (!network_disabled).then_some(DockAction::Network),
                ))
                .child(
                    self.arc_port_toggle(
                        cx,
                        "clipboard",
                        203.0,
                        IconName::Copy,
                        clipboard_active,
                        !clipboard_active,
                        theme::accent_blue(),
                        clipboard_tooltip.0,
                        clipboard_tooltip.1,
                        snapshot
                            .clipboard_control
                            .adopt_event_id
                            .map(DockAction::AdoptLatestClipboard),
                    ),
                ),
        )
    }
}
