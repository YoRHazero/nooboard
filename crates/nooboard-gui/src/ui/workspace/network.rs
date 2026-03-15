use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div, px};
use gpui_component::StyledExt;

use crate::ui::theme;

use super::{WorkspaceRenderModel, WorkspaceView};

impl WorkspaceView {
    pub(super) fn network_page(
        &self,
        model: &WorkspaceRenderModel,
        cx: &Context<Self>,
    ) -> Vec<AnyElement> {
        let state = model.page.as_ref();
        let network_status = state.map(|state| state.network_status.clone());
        let lan_peers = state.map(|state| state.lan_peers.clone()).unwrap_or_default();
        let direct_seeds = state.map(|state| state.direct_seeds.clone()).unwrap_or_default();
        let pending_requests = state
            .map(|state| state.pending_requests.clone())
            .unwrap_or_default();
        let sessions = state.map(|state| state.sessions.clone()).unwrap_or_default();

        vec![
            div()
                .h_flex()
                .gap(px(10.0))
                .child(self.toolbar_button(
                    "network-start",
                    "Start Network",
                    state.is_some_and(|state| state.network_can_start),
                    theme::accent_green(),
                    |this, _, _, cx| this.start_network_action(cx),
                    cx,
                ))
                .child(self.toolbar_button(
                    "network-stop",
                    "Stop Network",
                    state.is_some_and(|state| state.network_can_stop),
                    theme::accent_rose(),
                    |this, _, _, cx| this.stop_network_action(cx),
                    cx,
                ))
                .into_any_element(),
            self.list_card(
                "Network Status",
                &network_status.into_iter().collect::<Vec<_>>(),
                "Network status unavailable.",
            )
            .into_any_element(),
            self.list_card("LAN Peers", &lan_peers, "No LAN peers discovered.")
                .into_any_element(),
            self.list_card("Direct Seeds", &direct_seeds, "No direct seeds configured.")
                .into_any_element(),
            self.list_card(
                "Pending Direct Requests",
                &pending_requests,
                "No pending direct requests.",
            )
            .into_any_element(),
            self.list_card("Sessions", &sessions, "No active sessions.")
                .into_any_element(),
        ]
    }
}
