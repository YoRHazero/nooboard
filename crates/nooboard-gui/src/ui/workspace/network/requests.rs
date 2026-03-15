use gpui::{Context, IntoElement, ParentElement, Styled, div, px};
use gpui_component::{Disableable, StyledExt};

use crate::{
    ui::theme,
    workspace::view_state::{NetworkPageViewState, NetworkPendingRequestViewState},
};

use super::super::WorkspaceView;

impl WorkspaceView {
    pub(in crate::ui::workspace::network) fn network_request_panel(
        &self,
        state: &NetworkPageViewState,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        self.network_panel_shell(
            "Pending Direct Requests",
            format!("{} waiting", state.pending_request_count),
        )
        .children(if state.pending_requests.is_empty() {
            vec![
                self.network_empty_notice("No pending direct requests.")
                    .into_any_element(),
            ]
        } else {
            state
                .pending_requests
                .iter()
                .cloned()
                .map(|request| self.network_request_row(request, cx).into_any_element())
                .collect()
        })
    }

    pub(in crate::ui::workspace::network) fn network_request_row(
        &self,
        request: NetworkPendingRequestViewState,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let pending = self.network.request_pending(request.id);
        let approve_request = request.clone();
        let reject_request = request.clone();

        self.network_row_shell().child(
            div()
                .h_flex()
                .items_start()
                .justify_between()
                .gap(px(12.0))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .v_flex()
                        .gap(px(6.0))
                        .child(
                            div()
                                .text_size(px(14.0))
                                .font_semibold()
                                .text_color(theme::fg_primary())
                                .child(request.peer_device_id.clone()),
                        )
                        .child(
                            div()
                                .text_size(px(11.0))
                                .text_color(theme::fg_secondary())
                                .child(format!(
                                    "{} · {}",
                                    request.remote_addr_label, request.peer_noob_id
                                )),
                        )
                        .child(
                            div()
                                .text_size(px(11.0))
                                .text_color(theme::fg_muted())
                                .child(format!("expires at {}", request.expires_label)),
                        ),
                )
                .child(
                    div()
                        .h_flex()
                        .gap(px(8.0))
                        .child(
                            self.network_action_button(
                                format!("network-request-approve-{}", request.id),
                                "Approve",
                                theme::accent_green(),
                                cx,
                            )
                            .disabled(pending)
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    this.request_network_approve_request(&approve_request, cx);
                                },
                            )),
                        )
                        .child(
                            self.network_action_button(
                                format!("network-request-reject-{}", request.id),
                                "Reject",
                                theme::accent_rose(),
                                cx,
                            )
                            .disabled(pending)
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    this.request_network_reject_request(&reject_request, cx);
                                },
                            )),
                        ),
                ),
        )
    }
}
