use gpui::prelude::FluentBuilder as _;
use gpui::{
    Context, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled,
    div, px,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::{IconName, Sizable, StyledExt, scroll::ScrollableElement};

use crate::{
    ui::theme,
    workspace::view_state::{NetworkLanPeerViewState, NetworkPageViewState},
};

use super::super::WorkspaceView;

const LAN_LIST_HEIGHT: f32 = 320.0;

impl WorkspaceView {
    pub(in crate::ui::workspace::network) fn network_lan_panel(
        &self,
        state: &NetworkPageViewState,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let connected_peers = state
            .lan_peers
            .iter()
            .filter(|peer| peer.connected)
            .cloned()
            .collect::<Vec<_>>();

        self.network_panel_shell(
            "LAN Auto Sync",
            "Connected peers discovered through mDNS and managed automatically by runtime policy.",
        )
        .child(
            div()
                .h_flex()
                .items_center()
                .justify_between()
                .gap(px(12.0))
                .px(px(14.0))
                .py(px(12.0))
                .bg(theme::bg_console())
                .border_1()
                .border_color(theme::border_soft())
                .rounded(px(18.0))
                .child(
                    div()
                        .h_flex()
                        .items_center()
                        .gap(px(8.0))
                        .child(
                            div()
                                .text_size(px(12.0))
                                .font_semibold()
                                .text_color(theme::fg_primary())
                                .child("mDNS Discovery"),
                        )
                        .child(
                            div()
                                .id("network-lan-tooltip-shell")
                                .tooltip({
                                    let text = "Advertise this node and allow LAN auto-sync to discover matching peers.".to_string();
                                    move |window, cx| {
                                        Self::network_themed_tooltip(
                                            text.clone(),
                                            window,
                                            cx,
                                        )
                                    }
                                })
                                .child(
                                    Button::new("network-lan-tooltip")
                                        .ghost()
                                        .xsmall()
                                        .icon(IconName::Info),
                                ),
                        ),
                )
                .child(
                    div()
                        .h_flex()
                        .items_center()
                        .gap(px(10.0))
                        .child(
                            div()
                                .text_size(px(11.0))
                                .font_semibold()
                                .text_color(if state.lan_enabled {
                                    theme::accent_green()
                                } else {
                                    theme::fg_muted()
                                })
                                .child(if state.lan_enabled { "On" } else { "Off" }),
                        )
                        .child(self.network_inline_switch(
                            "network-lan-toggle",
                            state.lan_enabled,
                            theme::accent_green(),
                            cx.listener(|this, _, _, cx| {
                                this.request_network_toggle_lan_enabled(cx);
                            }),
                        )),
                ),
        )
        .child(
            div()
                .h(px(LAN_LIST_HEIGHT))
                .overflow_y_scrollbar()
                .child(
                    div()
                        .v_flex()
                        .gap(px(10.0))
                        .children(if !state.lan_enabled {
                            vec![
                                self.network_empty_notice("LAN auto sync is disabled.")
                                    .into_any_element(),
                            ]
                        } else if connected_peers.is_empty() {
                            vec![
                                self.network_empty_notice("No connected LAN peers.")
                                    .into_any_element(),
                            ]
                        } else {
                            connected_peers
                                .into_iter()
                                .map(|peer| self.network_lan_peer_card(peer).into_any_element())
                                .collect()
                        }),
                ),
        )
    }

    pub(in crate::ui::workspace::network) fn network_lan_peer_card(
        &self,
        peer: NetworkLanPeerViewState,
    ) -> impl IntoElement {
        self.network_row_shell().child(
            div()
                .v_flex()
                .gap(px(6.0))
                .child(
                    div()
                        .h_flex()
                        .items_center()
                        .gap(px(8.0))
                        .child(
                            div()
                                .text_size(px(14.0))
                                .font_semibold()
                                .text_color(theme::fg_primary())
                                .child(peer.device_id),
                        )
                        .child(self.network_status_chip(
                            "Connected",
                            "",
                            theme::accent_green(),
                        )),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(theme::fg_secondary())
                        .child(peer.noob_id),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(theme::fg_muted())
                        .child(peer.endpoint_label),
                )
                .when_some(peer.connected_at_label.clone(), |this, value| {
                    this.child(
                        div()
                            .text_size(px(11.0))
                            .text_color(theme::fg_muted())
                            .child(format!("connected at {value}")),
                    )
                })
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(theme::fg_muted())
                        .child(format!("last seen {}", peer.last_seen_label)),
                ),
        )
    }
}
