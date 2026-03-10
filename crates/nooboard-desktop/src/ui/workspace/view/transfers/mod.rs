mod activity;
mod components;
mod header;
mod page_state;
pub(super) mod snapshot;
mod uploads;

use std::collections::BTreeSet;

use gpui::{Context, Div, ParentElement, Styled, div, px};
use gpui_component::StyledExt;

use super::WorkspaceView;
use snapshot::build_transfers_snapshot;

pub(super) use page_state::TransfersPageState;

impl WorkspaceView {
    pub(super) fn transfers_page(&mut self, cx: &mut Context<Self>) -> Div {
        let connected_targets = self
            .live_store
            .read(cx)
            .app_state()
            .peers
            .connected
            .iter()
            .map(|peer| peer.noob_id.as_str().to_string())
            .collect::<BTreeSet<_>>();
        self.transfers_page_state
            .retain_connected_targets(&connected_targets);

        let snapshot = {
            let store = self.live_store.read(cx);
            build_transfers_snapshot(&store, &self.transfers_page_state.selected_target_noob_ids)
        };

        div()
            .w_full()
            .v_flex()
            .gap(px(18.0))
            .child(self.transfers_header(&snapshot))
            .child(self.transfers_target_panel(&snapshot, cx))
            .child(self.transfers_upload_panel(cx))
            .child(self.transfers_activity_panel(&snapshot, cx))
    }
}
