use gpui::{AnyElement, IntoElement};

use super::{WorkspaceRenderModel, WorkspaceView};

impl WorkspaceView {
    pub(super) fn transfers_page(&self, model: &WorkspaceRenderModel) -> Vec<AnyElement> {
        let state = model.page.as_ref();
        let incoming = state
            .map(|state| state.incoming_transfers.clone())
            .unwrap_or_default();
        let active = state
            .map(|state| state.active_transfers.clone())
            .unwrap_or_default();
        let completed = state
            .map(|state| state.completed_transfers.clone())
            .unwrap_or_default();

        vec![
            self.list_card("Incoming Transfers", &incoming, "No incoming transfers.")
                .into_any_element(),
            self.list_card("Active Transfers", &active, "No active transfers.")
                .into_any_element(),
            self.list_card("Completed Transfers", &completed, "No completed transfers.")
                .into_any_element(),
        ]
    }
}
