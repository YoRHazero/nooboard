mod components;
mod header;
mod list;
mod page_state;
mod snapshot;

use gpui::{Context, Div, Hsla, ParentElement, Styled, div, px};
use gpui_component::StyledExt;

use crate::ui::theme;

use self::components::{
    peer_status_badge, peers_empty_state, peers_filter_chip, peers_panel_header, peers_panel_shell,
    peers_summary_card, peers_table_header, peers_table_row,
};
use self::snapshot::{PeerVisualStatus, build_peers_snapshot};

use super::WorkspaceView;

pub(super) use page_state::PeersPageState;

impl WorkspaceView {
    pub(super) fn peers_page(&self, cx: &mut Context<Self>) -> Div {
        let live_store = self.live_store.read(cx);
        let snapshot = build_peers_snapshot(&live_store, self.peers_filter());

        div()
            .w_full()
            .v_flex()
            .gap(px(18.0))
            .child(self.peers_header(&snapshot, cx))
            .child(self.peers_list_panel(&snapshot))
    }

    fn peer_status_label(status: PeerVisualStatus) -> &'static str {
        match status {
            PeerVisualStatus::Connected => "Connected",
            PeerVisualStatus::Transferring => "Transferring",
        }
    }

    fn peer_status_accent(status: PeerVisualStatus) -> Hsla {
        match status {
            PeerVisualStatus::Connected => theme::accent_green(),
            PeerVisualStatus::Transferring => theme::accent_blue(),
        }
    }
}
