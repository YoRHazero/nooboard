use gpui::{Context, IntoElement, ParentElement, Styled, div, px};
use gpui_component::{Disableable, StyledExt, scroll::ScrollableElement};

use crate::{
    ui::theme,
    workspace::view_state::{NetworkPageViewState, NetworkPendingRequestViewState},
};

use super::super::WorkspaceView;

const DIRECT_LIST_HEIGHT: f32 = 420.0;

impl WorkspaceView {
    pub(in crate::ui::workspace::network) fn network_request_tab(
        &self,
        state: &NetworkPageViewState,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        div()
            .v_flex()
            .gap(px(10.0))
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(theme::fg_muted())
                    .child("Approve a request before a direct connection is opened."),
            )
            .child(
                div()
                    .h(px(DIRECT_LIST_HEIGHT))
                    .overflow_y_scrollbar()
                    .child(
                        div()
                            .v_flex()
                            .gap(px(10.0))
                            .children(if state.pending_requests.is_empty() {
                                vec![
                                    self.network_empty_notice("No connection requests waiting.")
                                        .into_any_element(),
                                ]
                            } else {
                                state
                                    .pending_requests
                                    .iter()
                                    .cloned()
                                    .map(|request| {
                                        self.network_request_card(request, cx).into_any_element()
                                    })
                                    .collect()
                            }),
                    ),
            )
    }

    pub(in crate::ui::workspace::network) fn network_request_card(
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
                                .h_flex()
                                .items_center()
                                .gap(px(8.0))
                                .child(
                                    div()
                                        .text_size(px(14.0))
                                        .font_semibold()
                                        .text_color(theme::fg_primary())
                                        .child(request.peer_device_id.clone()),
                                )
                                .child(self.network_status_chip(
                                    "Pending",
                                    "",
                                    theme::accent_amber(),
                                )),
                        )
                        .child(
                            div()
                                .text_size(px(11.0))
                                .text_color(theme::fg_secondary())
                                .child(request.peer_noob_id.clone()),
                        )
                        .child(
                            div()
                                .text_size(px(11.0))
                                .text_color(theme::fg_muted())
                                .child(format!(
                                    "{} · Expires at {}",
                                    request.remote_addr_label, request.expires_label
                                )),
                        ),
                )
                .child(
                    div()
                        .h_flex()
                        .gap(px(8.0))
                        .child(
                            self.network_action_button(
                                format!("network-request-approve-{}", request.id),
                                "Accept",
                                theme::accent_green(),
                                cx,
                            )
                            .disabled(pending)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.request_network_approve_request(&approve_request, cx);
                            })),
                        )
                        .child(
                            self.network_action_button(
                                format!("network-request-reject-{}", request.id),
                                "Reject",
                                theme::accent_rose(),
                                cx,
                            )
                            .disabled(pending)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.request_network_reject_request(&reject_request, cx);
                            })),
                        ),
                ),
        )
    }
}
