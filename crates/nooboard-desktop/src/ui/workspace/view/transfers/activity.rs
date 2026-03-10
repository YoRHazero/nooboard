use gpui::{Context, Div, ParentElement, Styled, div, prelude::FluentBuilder as _, px};
use gpui_component::progress::Progress;
use gpui_component::{Disableable, StyledExt};
use nooboard_app::IncomingTransferDisposition;

use crate::ui::theme;

use super::WorkspaceView;
use super::components::{
    transfer_action_button, transfer_card_heading, transfer_card_meta, transfer_status_badge,
    transfers_card_shell, transfers_panel_header, transfers_panel_shell, transfers_section,
};
use super::snapshot::{
    ActiveTransferCardSnapshot, CompletedTransferCardSnapshot, IncomingTransferCardSnapshot,
    TransferVisualAccent, TransfersSnapshot,
};

impl WorkspaceView {
    pub(super) fn transfers_activity_panel(
        &self,
        snapshot: &TransfersSnapshot,
        cx: &mut Context<Self>,
    ) -> Div {
        transfers_panel_shell()
            .gap(px(16.0))
            .child(transfers_panel_header(
                "Transfer Activity",
                format!(
                    "{} awaiting · {} active · {} completed",
                    snapshot.metrics.awaiting, snapshot.metrics.active, snapshot.metrics.completed
                ),
            ))
            .child(self.incoming_transfers_section(snapshot, cx))
            .child(self.active_uploads_section(snapshot, cx))
            .child(self.active_downloads_section(snapshot, cx))
            .child(self.completed_uploads_section(snapshot))
            .child(self.completed_downloads_section(snapshot))
    }

    fn incoming_transfers_section(
        &self,
        snapshot: &TransfersSnapshot,
        cx: &mut Context<Self>,
    ) -> Div {
        let cards = snapshot
            .incoming_pending
            .iter()
            .map(|item| self.incoming_transfer_card(item, cx))
            .collect::<Vec<_>>();

        transfers_section(
            "Incoming Transfers",
            cards.len(),
            cards,
            "No files are awaiting a local decision.",
        )
    }

    fn active_uploads_section(&self, snapshot: &TransfersSnapshot, cx: &mut Context<Self>) -> Div {
        let cards = snapshot
            .active_uploads
            .iter()
            .map(|item| self.active_transfer_card(item, cx))
            .collect::<Vec<_>>();

        transfers_section("Active Uploads", cards.len(), cards, "No active uploads.")
    }

    fn active_downloads_section(
        &self,
        snapshot: &TransfersSnapshot,
        cx: &mut Context<Self>,
    ) -> Div {
        let cards = snapshot
            .active_downloads
            .iter()
            .map(|item| self.active_transfer_card(item, cx))
            .collect::<Vec<_>>();

        transfers_section(
            "Active Downloads",
            cards.len(),
            cards,
            "No active downloads.",
        )
    }

    fn completed_uploads_section(&self, snapshot: &TransfersSnapshot) -> Div {
        let cards = snapshot
            .completed_uploads
            .iter()
            .map(Self::completed_transfer_card)
            .collect::<Vec<_>>();

        transfers_section(
            "Completed Uploads",
            cards.len(),
            cards,
            "No completed uploads.",
        )
    }

    fn completed_downloads_section(&self, snapshot: &TransfersSnapshot) -> Div {
        let cards = snapshot
            .completed_downloads
            .iter()
            .map(Self::completed_transfer_card)
            .collect::<Vec<_>>();

        transfers_section(
            "Completed Downloads",
            cards.len(),
            cards,
            "No completed downloads.",
        )
    }

    fn incoming_transfer_card(
        &self,
        item: &IncomingTransferCardSnapshot,
        cx: &mut Context<Self>,
    ) -> Div {
        let pending = self
            .transfers_page_state
            .transfer_action_pending(&item.transfer_id);
        let accept_id = item.transfer_id.clone();
        let reject_id = item.transfer_id.clone();

        transfers_card_shell()
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .justify_between()
                    .gap(px(10.0))
                    .child(transfer_card_heading(&item.file_name, theme::accent_amber()).flex_1())
                    .child(transfer_status_badge("Incoming", theme::accent_amber())),
            )
            .child(transfer_card_meta(
                &item.peer_device_id,
                &item.file_size_label,
            ))
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(theme::fg_muted())
                    .child(format!("Offered at {}", item.offered_at_label)),
            )
            .child(
                div()
                    .h_flex()
                    .gap(px(8.0))
                    .child(
                        transfer_action_button(
                            format!("incoming-transfer-accept-{}", item.transfer_id),
                            "Accept",
                            theme::accent_green(),
                            cx,
                        )
                        .disabled(pending)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.request_incoming_transfer_decision(
                                accept_id.clone(),
                                IncomingTransferDisposition::Accept,
                                cx,
                            );
                        })),
                    )
                    .child(
                        transfer_action_button(
                            format!("incoming-transfer-reject-{}", item.transfer_id),
                            "Reject",
                            theme::accent_rose(),
                            cx,
                        )
                        .disabled(pending)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.request_incoming_transfer_decision(
                                reject_id.clone(),
                                IncomingTransferDisposition::Reject,
                                cx,
                            );
                        })),
                    ),
            )
    }

    fn active_transfer_card(
        &self,
        item: &ActiveTransferCardSnapshot,
        cx: &mut Context<Self>,
    ) -> Div {
        let accent = transfer_accent_color(item.state_accent);
        let pending = self
            .transfers_page_state
            .transfer_action_pending(&item.transfer_id);
        let cancel_id = item.transfer_id.clone();
        let speed_label = item
            .speed_label
            .clone()
            .unwrap_or_else(|| fallback_estimate_label(item.state_label));
        let eta_label = item
            .eta_label
            .clone()
            .unwrap_or_else(|| fallback_estimate_label(item.state_label));

        transfers_card_shell()
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .justify_between()
                    .gap(px(10.0))
                    .child(transfer_card_heading(&item.file_name, accent).flex_1())
                    .child(transfer_status_badge(item.state_label, accent)),
            )
            .child(transfer_card_meta(
                &item.peer_device_id,
                &item.file_size_label,
            ))
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .justify_between()
                    .gap(px(10.0))
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(theme::fg_secondary())
                            .child(item.transferred_label.clone()),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .font_semibold()
                            .text_color(accent)
                            .child(item.progress_percent_label.clone()),
                    ),
            )
            .child(
                Progress::new(format!("transfer-progress-{}", item.transfer_id))
                    .value(item.progress_fraction * 100.0),
            )
            .child(
                div()
                    .grid()
                    .grid_cols(2)
                    .gap(px(10.0))
                    .child(transfer_detail_pair("Started", &item.started_at_label))
                    .child(transfer_detail_pair("Updated", &item.updated_at_label))
                    .child(transfer_detail_pair("Speed", &speed_label))
                    .child(transfer_detail_pair("Remaining", &eta_label)),
            )
            .child(
                div().h_flex().justify_end().child(
                    transfer_action_button(
                        format!("cancel-transfer-{}", item.transfer_id),
                        "Cancel",
                        theme::accent_rose(),
                        cx,
                    )
                    .disabled(pending || item.state_label == "Cancelling")
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.request_cancel_transfer(cancel_id.clone(), cx);
                    })),
                ),
            )
    }

    fn completed_transfer_card(item: &CompletedTransferCardSnapshot) -> Div {
        let accent = transfer_accent_color(item.outcome_accent);

        transfers_card_shell()
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .justify_between()
                    .gap(px(10.0))
                    .child(transfer_card_heading(&item.file_name, accent).flex_1())
                    .child(transfer_status_badge(item.outcome_label, accent)),
            )
            .child(transfer_card_meta(
                &item.peer_device_id,
                &item.file_size_label,
            ))
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(theme::fg_muted())
                    .child(match &item.duration_label {
                        Some(duration_label) => {
                            format!("Finished {} · {}", item.finished_at_label, duration_label)
                        }
                        None => format!("Finished {}", item.finished_at_label),
                    }),
            )
            .when_some(item.saved_path_label.clone(), |this, path| {
                this.child(
                    div()
                        .text_size(px(11.0))
                        .text_color(theme::accent_cyan())
                        .line_clamp(1)
                        .text_ellipsis()
                        .child(path),
                )
            })
            .when_some(item.message.clone(), |this, message| {
                this.child(
                    div()
                        .text_size(px(11.0))
                        .text_color(if item.outcome_accent == TransferVisualAccent::Green {
                            theme::fg_secondary()
                        } else {
                            theme::accent_rose()
                        })
                        .line_clamp(2)
                        .text_ellipsis()
                        .child(message),
                )
            })
    }
}

fn transfer_accent_color(accent: TransferVisualAccent) -> gpui::Hsla {
    match accent {
        TransferVisualAccent::Amber => theme::accent_amber(),
        TransferVisualAccent::Blue => theme::accent_blue(),
        TransferVisualAccent::Green => theme::accent_green(),
        TransferVisualAccent::Rose => theme::accent_rose(),
    }
}

fn transfer_detail_pair(label: &str, value: &str) -> Div {
    div()
        .v_flex()
        .gap(px(4.0))
        .child(
            div()
                .text_size(px(10.0))
                .font_semibold()
                .text_color(theme::fg_muted())
                .child(label.to_uppercase()),
        )
        .child(
            div()
                .text_size(px(11.0))
                .text_color(theme::fg_secondary())
                .line_clamp(1)
                .text_ellipsis()
                .child(value.to_string()),
        )
}

fn fallback_estimate_label(state_label: &str) -> String {
    match state_label {
        "In Progress" => "Estimating".to_string(),
        "Cancelling" => "Stopping".to_string(),
        _ => "Pending".to_string(),
    }
}
