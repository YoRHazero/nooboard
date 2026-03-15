use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled};
use gpui_component::StyledExt;

use crate::{
    ui::theme,
    workspace::{actions::network as network_actions, view_state::NetworkPageViewState},
};

use super::super::{WorkspaceRenderModel, WorkspaceView};

impl WorkspaceView {
    pub(in crate::ui::workspace) fn network_page(
        &self,
        model: &WorkspaceRenderModel,
        cx: &Context<Self>,
    ) -> Vec<AnyElement> {
        let Some(state) = model.page.as_ref().map(|page| &page.network) else {
            return vec![
                self.list_card(
                    "Network",
                    &["Waiting for workspace snapshot.".to_string()],
                    "Waiting for workspace snapshot.",
                )
                .into_any_element(),
            ];
        };

        vec![
            self.network_toolbar(state, cx).into_any_element(),
            self.network_summary_row(state).into_any_element(),
            self.network_seed_composer(cx).into_any_element(),
            self.network_seed_panel(state, cx).into_any_element(),
            self.network_request_panel(state, cx).into_any_element(),
            self.network_session_panel(state, cx).into_any_element(),
            self.network_lan_peer_panel(state).into_any_element(),
        ]
    }

    pub(in crate::ui::workspace::network) fn network_toolbar(
        &self,
        state: &NetworkPageViewState,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        gpui::div()
            .h_flex()
            .flex_wrap()
            .gap(gpui::px(10.0))
            .child(self.toolbar_button(
                "network-start",
                "Start Network",
                state.can_start,
                theme::accent_green(),
                |this, _, _, cx| network_actions::start_network(&this.controller, cx),
                cx,
            ))
            .child(self.toolbar_button(
                "network-stop",
                "Stop Network",
                state.can_stop,
                theme::accent_rose(),
                |this, _, _, cx| network_actions::stop_network(&this.controller, cx),
                cx,
            ))
            .child(self.network_status_chip("Status", &state.status_label, theme::accent_cyan()))
    }

    pub(in crate::ui::workspace::network) fn network_summary_row(
        &self,
        state: &NetworkPageViewState,
    ) -> impl IntoElement {
        gpui::div()
            .h_flex()
            .flex_wrap()
            .gap(gpui::px(12.0))
            .child(self.network_metric_card(
                "LAN Peers",
                state.lan_peer_count,
                theme::accent_green(),
            ))
            .child(self.network_metric_card(
                "Direct Seeds",
                state.direct_seed_count,
                theme::accent_blue(),
            ))
            .child(self.network_metric_card(
                "Requests",
                state.pending_request_count,
                theme::accent_amber(),
            ))
            .child(self.network_metric_card("Sessions", state.session_count, theme::accent_cyan()))
    }
}
