use gpui::prelude::FluentBuilder as _;
use gpui::{Context, IntoElement, ParentElement, Styled, div, px};
use gpui_component::{Disableable, StyledExt, scroll::ScrollableElement};

use crate::{
    ui::theme,
    workspace::view_state::{NetworkPageViewState, NetworkSessionViewState},
};

use super::super::WorkspaceView;

const DIRECT_LIST_HEIGHT: f32 = 420.0;

impl WorkspaceView {
    pub(in crate::ui::workspace::network) fn network_session_tab(
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
                    .child("These direct connections stay open until you disconnect them here."),
            )
            .child(
                div()
                    .h(px(DIRECT_LIST_HEIGHT))
                    .overflow_y_scrollbar()
                    .child(
                        div()
                            .v_flex()
                            .gap(px(10.0))
                            .children(if state.direct_sessions.is_empty() {
                                vec![
                                    self.network_empty_notice("No active direct connections.")
                                        .into_any_element(),
                                ]
                            } else {
                                state
                                    .direct_sessions
                                    .iter()
                                    .cloned()
                                    .map(|session| {
                                        self.network_session_card(session, cx).into_any_element()
                                    })
                                    .collect()
                            }),
                    ),
            )
    }

    pub(in crate::ui::workspace::network) fn network_session_card(
        &self,
        session: NetworkSessionViewState,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let pending = self.network.session_pending(session.id);
        let session_for_disconnect = session.clone();

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
                                        .child(session.peer_device_id.clone()),
                                )
                                .child(self.network_status_chip(
                                    "Direct",
                                    "",
                                    theme::accent_green(),
                                )),
                        )
                        .child(
                            div()
                                .text_size(px(11.0))
                                .text_color(theme::fg_secondary())
                                .child(session.peer_noob_id.clone()),
                        )
                        .child(
                            div()
                                .text_size(px(11.0))
                                .text_color(theme::fg_muted())
                                .child(format!("Remote {}", session.remote_addr_label)),
                        )
                        .when_some(session.local_bind_addr_label.clone(), |this, value| {
                            this.child(
                                div()
                                    .text_size(px(11.0))
                                    .text_color(theme::fg_muted())
                                    .child(format!("Local address {value}")),
                            )
                        })
                        .child(
                            div()
                                .text_size(px(11.0))
                                .text_color(theme::fg_muted())
                                .child(format!("Connected at {}", session.connected_at_label)),
                        ),
                )
                .child(
                    self.network_action_button(
                        format!("network-session-disconnect-{}", session.id),
                        "Disconnect",
                        theme::accent_rose(),
                        cx,
                    )
                    .disabled(pending)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.request_network_disconnect_session(&session_for_disconnect, cx);
                    })),
                ),
        )
    }
}
