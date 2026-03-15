use gpui::prelude::FluentBuilder as _;
use gpui::{Context, IntoElement, ParentElement, Styled, div, px};
use gpui_component::{Disableable, StyledExt};

use crate::{
    ui::theme,
    workspace::view_state::{
        ActiveTransferViewState, CompletedTransferViewState, IncomingTransferViewState,
        TransfersPageViewState,
    },
};

use super::super::WorkspaceView;

impl WorkspaceView {
    pub(in crate::ui::workspace::transfers) fn transfers_activity_panel(
        &self,
        state: &TransfersPageViewState,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        self.transfers_panel_shell(
            "Transfer Activity",
            format!(
                "{} incoming · {} active · {} complete",
                state.incoming.len(),
                state.active.len(),
                state.completed.len()
            ),
        )
        .child(
            self.transfer_section(
                "Incoming Transfers",
                state.incoming.len(),
                state
                    .incoming
                    .iter()
                    .cloned()
                    .map(|transfer| self.incoming_transfer_card(transfer, cx).into_any_element())
                    .collect(),
                "No incoming transfers.",
            ),
        )
        .child(
            self.transfer_section(
                "Active Transfers",
                state.active.len(),
                state
                    .active
                    .iter()
                    .cloned()
                    .map(|transfer| self.active_transfer_card(transfer, cx).into_any_element())
                    .collect(),
                "No active transfers.",
            ),
        )
        .child(
            self.transfer_section(
                "Completed Transfers",
                state.completed.len(),
                state
                    .completed
                    .iter()
                    .cloned()
                    .map(|transfer| self.completed_transfer_card(transfer).into_any_element())
                    .collect(),
                "No completed transfers.",
            ),
        )
    }

    pub(in crate::ui::workspace::transfers) fn incoming_transfer_card(
        &self,
        transfer: IncomingTransferViewState,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let pending = self.transfers.pending_ticket(transfer.ticket);
        let accept_transfer = transfer.clone();
        let reject_transfer = transfer.clone();

        self.transfers_card_shell()
            .child(self.transfer_card_heading(&transfer.file_name, theme::accent_amber()))
            .child(self.transfer_card_meta(&transfer.peer_device_id, &transfer.size_label))
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(theme::fg_muted())
                    .child(transfer.peer_noob_id.clone()),
            )
            .child(
                div()
                    .h_flex()
                    .justify_end()
                    .gap(px(8.0))
                    .child(
                        self.transfer_action_button(
                            format!("transfer-reject-{}", transfer.ticket.raw_id),
                            "Reject",
                            theme::accent_rose(),
                            cx,
                        )
                        .disabled(pending)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.request_transfer_decision(&reject_transfer, false, cx);
                        })),
                    )
                    .child(
                        self.transfer_action_button(
                            format!("transfer-accept-{}", transfer.ticket.raw_id),
                            "Accept",
                            theme::accent_green(),
                            cx,
                        )
                        .disabled(pending)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.request_transfer_decision(&accept_transfer, true, cx);
                        })),
                    ),
            )
    }

    pub(in crate::ui::workspace::transfers) fn active_transfer_card(
        &self,
        transfer: ActiveTransferViewState,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let pending = self.transfers.pending_ticket(transfer.ticket);
        let cancel_transfer = transfer.clone();

        self.transfers_card_shell()
            .child(self.transfer_card_heading(&transfer.file_name, theme::accent_cyan()))
            .child(self.transfer_card_meta(&transfer.peer_device_id, &transfer.progress_label))
            .child(
                div()
                    .h_flex()
                    .gap(px(8.0))
                    .child(
                        self.transfer_status_badge(&transfer.direction_label, theme::accent_blue()),
                    )
                    .child(self.transfer_status_badge(&transfer.state_label, theme::accent_cyan())),
            )
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(theme::fg_muted())
                    .child(transfer.peer_noob_id.clone()),
            )
            .child(
                div().h_flex().justify_end().child(
                    self.transfer_action_button(
                        format!("transfer-cancel-{}", transfer.ticket.raw_id),
                        "Cancel",
                        theme::accent_rose(),
                        cx,
                    )
                    .disabled(pending)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.request_transfer_cancel(&cancel_transfer, cx);
                    })),
                ),
            )
    }

    pub(in crate::ui::workspace::transfers) fn completed_transfer_card(
        &self,
        transfer: CompletedTransferViewState,
    ) -> impl IntoElement {
        self.transfers_card_shell()
            .child(self.transfer_card_heading(&transfer.file_name, theme::accent_green()))
            .child(self.transfer_card_meta(&transfer.peer_device_id, &transfer.direction_label))
            .child(self.transfer_status_badge(&transfer.outcome_label, theme::accent_green()))
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(theme::fg_muted())
                    .line_clamp(1)
                    .text_ellipsis()
                    .child(format!(
                        "{} · ticket {}:{}",
                        transfer.peer_noob_id, transfer.ticket.session_id, transfer.ticket.raw_id
                    )),
            )
            .when_some(transfer.saved_path.clone(), |this, path| {
                this.child(
                    div()
                        .text_size(px(11.0))
                        .text_color(theme::fg_muted())
                        .line_clamp(1)
                        .text_ellipsis()
                        .child(path.display().to_string()),
                )
            })
            .when_some(transfer.message.clone(), |this, message| {
                this.child(
                    div()
                        .text_size(px(11.0))
                        .text_color(theme::fg_muted())
                        .line_clamp(2)
                        .text_ellipsis()
                        .child(message),
                )
            })
    }
}
