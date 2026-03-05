mod header;
mod list;
mod page_state;

use gpui::{Context, Div, Hsla, ParentElement, Styled, div, px};
use gpui_component::StyledExt;

use crate::{
    state::{SystemPeer, SystemPeerStatus},
    ui::theme,
};

use super::WorkspaceView;

pub(super) use page_state::PeersPageState;

impl WorkspaceView {
    pub(super) fn peers_page(&self, cx: &mut Context<Self>) -> Div {
        div()
            .w_full()
            .v_flex()
            .gap(px(18.0))
            .child(self.peers_header(cx))
            .child(self.peers_list_panel())
    }

    fn peer_counts(&self) -> (usize, usize, usize) {
        self.state.app.system_core.peers.iter().fold(
            (0usize, 0usize, 0usize),
            |(total, connected, transferring), peer| match peer.status {
                SystemPeerStatus::Connected => (total + 1, connected + 1, transferring),
                SystemPeerStatus::Transferring => (total + 1, connected, transferring + 1),
            },
        )
    }

    fn filtered_peers(&self) -> Vec<&SystemPeer> {
        let filter = self.peers_filter();

        self.state
            .app
            .system_core
            .peers
            .iter()
            .filter(|peer| filter.matches(peer.status))
            .collect()
    }

    fn peer_status_label(status: SystemPeerStatus) -> &'static str {
        match status {
            SystemPeerStatus::Connected => "Connected",
            SystemPeerStatus::Transferring => "Transferring",
        }
    }

    fn peer_status_accent(status: SystemPeerStatus) -> Hsla {
        match status {
            SystemPeerStatus::Connected => theme::accent_green(),
            SystemPeerStatus::Transferring => theme::accent_blue(),
        }
    }
}
