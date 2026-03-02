use gpui::{AnimationExt as _, Div, Hsla, IntoElement, ParentElement, Styled, div, px};
use gpui_component::{Icon, IconName, StyledExt};

use crate::ui::theme;

use super::super::{
    WorkspaceView,
    components::{command_bar, console_pill, data_cell, pulse_beacon, section_header},
    shared::{enter_animation, pulse_animation},
};

impl WorkspaceView {
    fn runtime_radar_node(
        &self,
        id: &'static str,
        top: f32,
        left: f32,
        label: &str,
        icon: IconName,
        accent: Hsla,
    ) -> impl IntoElement {
        div()
            .absolute()
            .top(px(top))
            .left(px(left))
            .v_flex()
            .items_center()
            .gap(px(8.0))
            .child(
                div()
                    .size(px(34.0))
                    .rounded(px(14.0))
                    .bg(theme::bg_panel())
                    .border_1()
                    .border_color(accent.opacity(0.34))
                    .shadow_xs()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(Icon::new(icon).size(px(15.0)).text_color(accent))
                    .with_animation(id, pulse_animation(), move |this, delta| {
                        this.bg(accent.opacity(0.1 + delta * 0.1))
                            .border_color(accent.opacity(0.22 + delta * 0.24))
                            .opacity(0.7 + delta * 0.3)
                    }),
            )
            .child(
                div()
                    .text_size(px(10.0))
                    .font_semibold()
                    .text_color(theme::fg_secondary())
                    .child(label.to_string()),
            )
    }

    fn runtime_signal_row(&self, label: &str, value: &str, detail: &str, accent: Hsla) -> Div {
        div()
            .h_flex()
            .items_start()
            .justify_between()
            .gap(px(14.0))
            .p(px(12.0))
            .bg(theme::bg_panel_alt())
            .border_1()
            .border_color(theme::border_base())
            .rounded(px(16.0))
            .child(
                div()
                    .h_flex()
                    .items_start()
                    .gap(px(10.0))
                    .child(div().size(px(8.0)).rounded(px(999.0)).bg(accent))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .v_flex()
                            .gap(px(4.0))
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .font_semibold()
                                    .text_color(theme::fg_secondary())
                                    .child(label.to_uppercase()),
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(theme::fg_muted())
                                    .line_clamp(2)
                                    .text_ellipsis()
                                    .child(detail.to_string()),
                            ),
                    ),
            )
            .child(
                div()
                    .text_size(px(12.0))
                    .font_semibold()
                    .text_color(theme::fg_primary())
                    .child(value.to_string()),
            )
    }

    fn runtime_radar_panel(&self) -> Div {
        div()
            .w(px(272.0))
            .flex_shrink_0()
            .v_flex()
            .gap(px(14.0))
            .child(
                div()
                    .relative()
                    .h(px(280.0))
                    .overflow_hidden()
                    .bg(theme::bg_console())
                    .border_1()
                    .border_color(theme::border_strong())
                    .rounded(px(24.0))
                    .child(
                        div()
                            .absolute()
                            .top(px(14.0))
                            .left(px(16.0))
                            .right(px(16.0))
                            .h_flex()
                            .justify_between()
                            .items_center()
                            .gap(px(12.0))
                            .child(
                                div()
                                    .v_flex()
                                    .gap(px(4.0))
                                    .child(
                                        div()
                                            .text_size(px(10.0))
                                            .font_semibold()
                                            .text_color(theme::accent_cyan())
                                            .child("RUNTIME RADAR"),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11.0))
                                            .text_color(theme::fg_muted())
                                            .child("control-plane sweep"),
                                    ),
                            )
                            .child(console_pill("nominal", theme::accent_green())),
                    )
                    .child(
                        div()
                            .absolute()
                            .top(px(48.0))
                            .left(px(18.0))
                            .right(px(18.0))
                            .h(px(1.0))
                            .bg(theme::border_base().opacity(0.65)),
                    )
                    .child(
                        div()
                            .absolute()
                            .top(px(0.0))
                            .left(px(0.0))
                            .right(px(0.0))
                            .bottom(px(0.0))
                            .bg(theme::accent_cyan().opacity(0.03)),
                    )
                    .child(
                        div()
                            .absolute()
                            .top(px(68.0))
                            .left(px(20.0))
                            .right(px(20.0))
                            .h(px(42.0))
                            .bg(theme::accent_cyan().opacity(0.05))
                            .border_1()
                            .border_color(theme::accent_cyan().opacity(0.1))
                            .rounded(px(12.0))
                            .with_animation("runtime-radar-scan-band", pulse_animation(), |this, delta| {
                                this.opacity(0.08 + delta * 0.28)
                            }),
                    )
                    .child(
                        div()
                            .absolute()
                            .top(px(48.0))
                            .bottom(px(58.0))
                            .left(px(135.0))
                            .w(px(1.0))
                            .bg(theme::border_soft().opacity(0.7)),
                    )
                    .child(
                        div()
                            .absolute()
                            .top(px(139.0))
                            .left(px(28.0))
                            .right(px(28.0))
                            .h(px(1.0))
                            .bg(theme::border_soft().opacity(0.7)),
                    )
                    .child(
                        div()
                            .absolute()
                            .top(px(74.0))
                            .left(px(54.0))
                            .w(px(164.0))
                            .h(px(1.0))
                            .bg(theme::border_base().opacity(0.25)),
                    )
                    .child(
                        div()
                            .absolute()
                            .top(px(106.0))
                            .left(px(42.0))
                            .w(px(188.0))
                            .h(px(1.0))
                            .bg(theme::border_base().opacity(0.22)),
                    )
                    .child(
                        div()
                            .absolute()
                            .top(px(172.0))
                            .left(px(42.0))
                            .w(px(188.0))
                            .h(px(1.0))
                            .bg(theme::border_base().opacity(0.22)),
                    )
                    .child(
                        div()
                            .absolute()
                            .top(px(32.0))
                            .left(px(50.0))
                            .size(px(172.0))
                            .rounded(px(999.0))
                            .border_1()
                            .border_color(theme::border_base().opacity(0.7)),
                    )
                    .child(
                        div()
                            .absolute()
                            .top(px(56.0))
                            .left(px(74.0))
                            .size(px(124.0))
                            .rounded(px(999.0))
                            .border_1()
                            .border_color(theme::border_base().opacity(0.55)),
                    )
                    .child(
                        div()
                            .absolute()
                            .top(px(28.0))
                            .right(px(22.0))
                            .v_flex()
                            .items_end()
                            .gap(px(26.0))
                            .child(
                                div()
                                    .text_size(px(9.0))
                                    .font_semibold()
                                    .text_color(theme::fg_muted())
                                    .child("R3"),
                            )
                            .child(
                                div()
                                    .text_size(px(9.0))
                                    .font_semibold()
                                    .text_color(theme::fg_muted())
                                    .child("R2"),
                            )
                            .child(
                                div()
                                    .text_size(px(9.0))
                                    .font_semibold()
                                    .text_color(theme::fg_muted())
                                    .child("R1"),
                            ),
                    )
                    .child(
                        div()
                            .absolute()
                            .top(px(82.0))
                            .left(px(100.0))
                            .size(px(72.0))
                            .rounded(px(999.0))
                            .border_1()
                            .border_color(theme::accent_green().opacity(0.35)),
                    )
                    .child(
                        div()
                            .absolute()
                            .top(px(116.0))
                            .left(px(116.0))
                            .size(px(40.0))
                            .rounded(px(999.0))
                            .bg(theme::accent_green().opacity(0.16))
                            .border_1()
                            .border_color(theme::accent_green().opacity(0.4))
                            .shadow_xs()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                Icon::new(IconName::LayoutDashboard)
                                    .size(px(16.0))
                                    .text_color(theme::accent_green()),
                            )
                            .with_animation("runtime-radar-core", pulse_animation(), |this, delta| {
                                this.bg(theme::accent_green().opacity(0.12 + delta * 0.12))
                                    .border_color(theme::accent_green().opacity(0.28 + delta * 0.3))
                                    .opacity(0.72 + delta * 0.28)
                            }),
                    )
                    .child(
                        div()
                            .absolute()
                            .top(px(132.0))
                            .left(px(108.0))
                            .w(px(26.0))
                            .h(px(1.0))
                            .bg(theme::border_strong().opacity(0.55)),
                    )
                    .child(
                        div()
                            .absolute()
                            .top(px(102.0))
                            .left(px(150.0))
                            .w(px(22.0))
                            .h(px(1.0))
                            .bg(theme::border_strong().opacity(0.55)),
                    )
                    .child(
                        div()
                            .absolute()
                            .top(px(158.0))
                            .left(px(148.0))
                            .w(px(22.0))
                            .h(px(1.0))
                            .bg(theme::border_strong().opacity(0.55)),
                    )
                    .child(self.runtime_radar_node(
                        "runtime-radar-transport",
                        86.0,
                        178.0,
                        "transport",
                        IconName::Globe,
                        theme::accent_cyan(),
                    ))
                    .child(self.runtime_radar_node(
                        "runtime-radar-discovery",
                        150.0,
                        176.0,
                        "discovery",
                        IconName::Bell,
                        theme::accent_green(),
                    ))
                    .child(self.runtime_radar_node(
                        "runtime-radar-clipboard",
                        120.0,
                        32.0,
                        "clipboard",
                        IconName::Copy,
                        theme::accent_blue(),
                    ))
                    .child(
                        div()
                            .absolute()
                            .left(px(16.0))
                            .right(px(16.0))
                            .bottom(px(16.0))
                            .v_flex()
                            .gap(px(10.0))
                            .p(px(10.0))
                            .bg(theme::bg_canvas().opacity(0.75))
                            .border_1()
                            .border_color(theme::border_base())
                            .rounded(px(16.0))
                            .child(
                                div()
                                    .h_flex()
                                    .justify_between()
                                    .items_center()
                                    .gap(px(12.0))
                                    .child(
                                        div()
                                            .v_flex()
                                            .gap(px(2.0))
                                            .child(
                                                div()
                                                    .text_size(px(10.0))
                                                    .font_semibold()
                                                    .text_color(theme::fg_secondary())
                                                    .child("CORE STATE"),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(13.0))
                                                    .font_semibold()
                                                    .text_color(theme::fg_primary())
                                                    .child(self.sync_label()),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .h_flex()
                                            .gap(px(8.0))
                                            .items_center()
                                            .child(pulse_beacon(
                                                "runtime-radar-footer",
                                                theme::accent_green(),
                                            ))
                                            .child(
                                                div()
                                                    .text_size(px(11.0))
                                                    .text_color(theme::fg_secondary())
                                                    .child("control plane nominal"),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .h_flex()
                                    .gap(px(8.0))
                                    .items_center()
                                    .child(console_pill("transport", theme::accent_cyan()))
                                    .child(console_pill("discovery", theme::accent_green()))
                                    .child(console_pill("clipboard", theme::accent_blue())),
                            ),
                    ),
            )
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(theme::fg_muted())
                    .line_clamp(2)
                    .text_ellipsis()
                    .child("A denser control-room radar for subsystem posture, scan coverage, and route readiness. It stays interpretive so the top summary strip remains the only quantity-focused layer."),
            )
    }

    pub(super) fn system_core_card(&self) -> impl IntoElement {
        let latest_clipboard = self
            .state
            .app
            .recent_history
            .first()
            .cloned()
            .unwrap_or_else(|| "no clipboard snapshot recorded yet".into());
        let latest_event_title = self
            .state
            .app
            .recent_activity
            .first()
            .map(|item| item.title.clone())
            .unwrap_or_else(|| "No recent runtime event has been recorded yet.".into());
        let latest_event_detail = self
            .state
            .app
            .recent_activity
            .first()
            .map(|item| item.detail.clone())
            .unwrap_or_else(|| {
                "The control plane is waiting for the next clipboard or transfer signal.".into()
            });

        div()
            .v_flex()
            .gap(px(20.0))
            .p(px(22.0))
            .bg(theme::bg_panel())
            .border_1()
            .border_color(theme::border_base())
            .rounded(px(24.0))
            .shadow_xs()
            .child(command_bar(
                "control plane",
                "interpreting runtime posture instead of duplicating traffic counts",
                theme::accent_green(),
            ))
            .child(section_header(
                "Runtime Health",
                "System Core",
                "Control-plane interpretation for the current desktop runtime. This card explains system posture instead of repeating traffic counts from the summary row.",
                theme::accent_green(),
            ))
            .child(
                div()
                    .h_flex()
                    .items_start()
                    .gap(px(18.0))
                    .child(self.runtime_radar_panel())
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .v_flex()
                            .gap(px(14.0))
                            .child(
                                div()
                                    .grid()
                                    .grid_cols(2)
                                    .gap(px(12.0))
                                    .child(data_cell(
                                        "Desired State",
                                        self.desired_state_label(),
                                        "control target",
                                        theme::accent_green(),
                                    ))
                                    .child(data_cell(
                                        "Runtime Status",
                                        self.sync_label(),
                                        "desktop worker posture",
                                        theme::accent_cyan(),
                                    ))
                                    .child(data_cell(
                                        "Transport Link",
                                        "ONLINE",
                                        "clipboard transport channel is available",
                                        theme::accent_blue(),
                                    ))
                                    .child(data_cell(
                                        "Discovery",
                                        "mDNS READY",
                                        "service beaconing is healthy",
                                        theme::accent_green(),
                                    )),
                            )
                            .child(
                                div()
                                    .v_flex()
                                    .gap(px(12.0))
                                    .p(px(16.0))
                                    .bg(theme::bg_console())
                                    .border_1()
                                    .border_color(theme::border_soft())
                                    .rounded(px(20.0))
                                    .child(
                                        div()
                                            .h_flex()
                                            .justify_between()
                                            .items_center()
                                            .gap(px(12.0))
                                            .child(
                                                div()
                                                    .text_size(px(11.0))
                                                    .font_semibold()
                                                    .text_color(theme::fg_secondary())
                                                    .child("SERVICE SIGNALS"),
                                            )
                                            .child(console_pill("subsystems", theme::accent_cyan())),
                                    )
                                    .child(self.runtime_signal_row(
                                        "Clipboard Mirror",
                                        "READY",
                                        "latest snapshot is staged for rebroadcast and local paste recovery",
                                        theme::accent_blue(),
                                    ))
                                    .child(self.runtime_signal_row(
                                        "Retry Budget",
                                        "CALM",
                                        "the last anomaly is aging out and no reconnection storm is visible",
                                        theme::accent_amber(),
                                    ))
                                    .child(self.runtime_signal_row(
                                        "Operator Focus",
                                        "STABLE",
                                        "home dashboard is clear enough for supervision without opening secondary panels",
                                        theme::accent_cyan(),
                                    )),
                            )
                            .child(
                                div()
                                    .v_flex()
                                    .gap(px(12.0))
                                    .p(px(16.0))
                                    .bg(theme::bg_panel_highlight())
                                    .border_1()
                                    .border_color(theme::border_soft())
                                    .rounded(px(20.0))
                                    .child(
                                        div()
                                            .h_flex()
                                            .justify_between()
                                            .items_center()
                                            .gap(px(12.0))
                                            .child(
                                                div()
                                                    .text_size(px(11.0))
                                                    .font_semibold()
                                                    .text_color(theme::fg_secondary())
                                                    .child("CONTROL NARRATIVE"),
                                            )
                                            .child(console_pill("operator read", theme::accent_blue())),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(14.0))
                                            .font_semibold()
                                            .text_color(theme::fg_primary())
                                            .line_clamp(2)
                                            .text_ellipsis()
                                            .child(latest_event_title),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .text_color(theme::fg_secondary())
                                            .line_clamp(3)
                                            .text_ellipsis()
                                            .child(latest_event_detail),
                                    )
                                    .child(div().h(px(1.0)).w_full().bg(theme::border_soft()))
                                    .child(
                                        div()
                                            .text_size(px(11.0))
                                            .font_semibold()
                                            .text_color(theme::fg_secondary())
                                            .child("LATEST CLIPBOARD SNAPSHOT"),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(13.0))
                                            .text_color(theme::fg_primary())
                                            .line_clamp(3)
                                            .text_ellipsis()
                                            .child(latest_clipboard),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .text_color(theme::fg_muted())
                                            .child("This area explains current runtime posture and recent control-plane context, so the quantity layer remains isolated to the top summary strip."),
                                    ),
                            ),
                    ),
            )
            .with_animation("system-core-card", enter_animation(), |this, delta| {
                this.opacity(0.35 + delta * 0.65)
            })
    }
}
