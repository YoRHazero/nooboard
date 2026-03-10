use gpui::Context;

use super::WorkspaceView;
use super::snapshot::PeerVisualStatus;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum PeersFilter {
    All,
    Idle,
    Transferring,
}

impl PeersFilter {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Idle => "Idle",
            Self::Transferring => "Transferring",
        }
    }

    pub(super) fn matches(self, status: PeerVisualStatus) -> bool {
        match self {
            Self::All => true,
            Self::Idle => status == PeerVisualStatus::Connected,
            Self::Transferring => status == PeerVisualStatus::Transferring,
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
