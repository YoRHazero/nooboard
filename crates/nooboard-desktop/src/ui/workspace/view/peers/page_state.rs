use gpui::Context;

use crate::state::SystemPeerStatus;

use super::WorkspaceView;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum PeersFilter {
    All,
    Connected,
    Transferring,
}

impl PeersFilter {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Connected => "Connected",
            Self::Transferring => "Transferring",
        }
    }

    pub(super) fn matches(self, status: SystemPeerStatus) -> bool {
        match self {
            Self::All => true,
            Self::Connected => status == SystemPeerStatus::Connected,
            Self::Transferring => status == SystemPeerStatus::Transferring,
        }
    }
}

pub(in crate::ui::workspace::view) struct PeersPageState {
    pub(super) filter: PeersFilter,
}

impl PeersPageState {
    pub(in crate::ui::workspace::view) fn new() -> Self {
        Self {
            filter: PeersFilter::All,
        }
    }
}

impl WorkspaceView {
    pub(super) fn peers_filter(&self) -> PeersFilter {
        self.peers_page_state.filter
    }

    pub(super) fn set_peers_filter(&mut self, filter: PeersFilter, cx: &mut Context<Self>) {
        if self.peers_page_state.filter == filter {
            return;
        }

        self.peers_page_state.filter = filter;
        cx.notify();
    }
}
