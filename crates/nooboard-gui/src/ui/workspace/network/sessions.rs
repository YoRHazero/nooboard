use gpui::prelude::FluentBuilder as _;
use gpui::{Context, IntoElement, ParentElement, Styled, div, px};
use gpui_component::{Disableable, StyledExt};

use crate::{
    ui::theme,
    workspace::view_state::{NetworkPageViewState, NetworkSessionViewState},
};

use super::super::WorkspaceView;

impl WorkspaceView {
    pub(in crate::ui::workspace::network) fn network_session_panel(
        &self,
        state: &NetworkPageViewState,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        self.network_panel_shell("Sessions", format!("{} active", state.session_count))
            .children(if state.sessions.is_empty() {
                vec![
                    self.network_empty_notice("No active sessions.")
                        .into_any_element(),
                ]
            } else {
                state
                    .sessions
                    .iter()
                    .cloned()
                    .map(|session| self.network_session_row(session, cx).into_any_element())
                    .collect()
            })
    }

    pub(in crate::ui::workspace::network) fn network_session_row(
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
                                    &session.mode_label,
                                    &session.remote_addr_label,
                                    theme::accent_cyan(),
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
                                .child(format!("connected at {}", session.connected_at_label)),
                        )
                        .when_some(session.local_bind_addr_label.clone(), |this, value| {
                            this.child(
                                div()
                                    .text_size(px(11.0))
                                    .text_color(theme::fg_muted())
                                    .child(format!("local bind {value}")),
                            )
                        }),
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
