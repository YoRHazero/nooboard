use gpui::{Div, IntoElement, ParentElement, Styled, div, px};
use gpui_component::StyledExt;

use super::{
    WorkspaceView, peer_status_badge, peers_empty_state, peers_panel_header, peers_panel_shell,
    peers_table_header, peers_table_row,
};

impl WorkspaceView {
    pub(super) fn peers_list_panel(&self) -> Div {
        let peers = self.filtered_peers();
        let filter = self.peers_filter();
        let count = peers.len();

        peers_panel_shell()
            .rounded(px(24.0))
            .w_full()
            .v_flex()
            .gap(px(12.0))
            .p(px(18.0))
            .child(peers_panel_header(
                "Connected tab",
                format!("{} · {} peers", filter.label(), count),
            ))
            .child(
                div()
                    .h(px(1.0))
                    .w_full()
                    .bg(crate::ui::theme::border_soft()),
            )
            .child(if peers.is_empty() {
                peers_empty_state(filter.label()).into_any_element()
            } else {
                div()
                    .v_flex()
                    .gap(px(8.0))
                    .child(peers_table_header())
                    .children(peers.into_iter().enumerate().map(|(index, peer)| {
                        peers_table_row(
                            index,
                            peer.device_id.clone(),
                            peer.noob_id.clone(),
                            peer.ip.clone(),
                            peer_status_badge(
                                Self::peer_status_label(peer.status),
                                Self::peer_status_accent(peer.status),
                            ),
                        )
                    }))
                    .into_any_element()
            })
    }
}
