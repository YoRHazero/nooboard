use gpui::{IntoElement, ParentElement, Styled, div, px};
use gpui_component::StyledExt;

use crate::{
    ui::theme,
    workspace::view_state::{NetworkLanPeerViewState, NetworkPageViewState},
};

use super::super::WorkspaceView;

impl WorkspaceView {
    pub(in crate::ui::workspace::network) fn network_lan_peer_panel(
        &self,
        state: &NetworkPageViewState,
    ) -> impl IntoElement {
        self.network_panel_shell("LAN Peers", format!("{} visible", state.lan_peer_count))
            .children(if state.lan_peers.is_empty() {
                vec![
                    self.network_empty_notice("No LAN peers discovered.")
                        .into_any_element(),
                ]
            } else {
                state
                    .lan_peers
                    .iter()
                    .cloned()
                    .map(|peer| self.network_lan_peer_row(peer).into_any_element())
                    .collect()
            })
    }

    pub(in crate::ui::workspace::network) fn network_lan_peer_row(
        &self,
        peer: NetworkLanPeerViewState,
    ) -> impl IntoElement {
        self.network_row_shell().child(
            div()
                .h_flex()
                .items_start()
                .justify_between()
                .gap(px(12.0))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .v_flex()
                        .gap(px(6.0))
                        .child(
                            div()
                                .text_size(px(14.0))
                                .font_semibold()
                                .text_color(theme::fg_primary())
                                .child(peer.device_id),
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
                                .child(format!(
                                    "{} · last seen {}",
                                    peer.endpoint_label, peer.last_seen_label
                                )),
                        ),
                )
                .child(self.network_status_chip(
                    if peer.connected { "Connected" } else { "Seen" },
                    "",
                    if peer.connected {
                        theme::accent_green()
                    } else {
                        theme::accent_cyan()
                    },
                )),
        )
    }
}
