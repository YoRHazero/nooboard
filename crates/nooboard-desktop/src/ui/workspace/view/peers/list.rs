use gpui::{Div, IntoElement, ParentElement, Styled, div, px};
use gpui_component::StyledExt;

use super::snapshot::PeersSnapshot;
use super::{
    WorkspaceView, peer_status_badge, peers_empty_state, peers_panel_header, peers_panel_shell,
    peers_table_header, peers_table_row,
};

impl WorkspaceView {
    pub(super) fn peers_list_panel(&self, snapshot: &PeersSnapshot) -> Div {
        let count = snapshot.rows.len();

        peers_panel_shell()
            .rounded(px(24.0))
            .w_full()
            .v_flex()
            .gap(px(12.0))
            .p(px(18.0))
            .child(peers_panel_header(
                "Connected peers",
                format!("{} · {} peers", snapshot.filter.label(), count),
            ))
            .child(
                div()
                    .h(px(1.0))
                    .w_full()
                    .bg(crate::ui::theme::border_soft()),
            )
            .child(if snapshot.rows.is_empty() {
                peers_empty_state(snapshot.filter.label()).into_any_element()
            } else {
                div()
                    .v_flex()
                    .gap(px(8.0))
                    .child(peers_table_header())
                    .children(snapshot.rows.iter().enumerate().map(|(index, peer)| {
                        peers_table_row(
                            index,
                            peer.device_id.clone(),
                            peer.noob_id.clone(),
                            peer.endpoint_label.clone(),
                            peer.endpoint_detail.clone(),
                            peer.duplicate_device_id,
                            peer.duplicates_local_identity,
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
