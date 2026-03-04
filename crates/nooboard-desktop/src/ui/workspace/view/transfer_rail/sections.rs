use gpui::{Context, Div, ParentElement, Styled, div, px};
use gpui_component::StyledExt;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::progress::Progress;

use crate::state::{TransferRailItem, TransferRailStatus};
use crate::ui::theme;

use super::WorkspaceView;

impl WorkspaceView {
    pub(super) fn transfer_sections(&self, cx: &mut Context<Self>) -> Div {
        div()
            .v_flex()
            .gap(px(18.0))
            .child(self.awaiting_review_section(cx))
            .child(self.in_progress_section())
            .child(self.completed_section(cx))
    }

    fn awaiting_review_section(&self, cx: &mut Context<Self>) -> Div {
        let cards = self
            .transfer_rail_items
            .iter()
            .filter(|item| item.is_awaiting_review())
            .enumerate()
            .map(|(index, item)| self.awaiting_review_card(index, item, cx))
            .collect::<Vec<_>>();

        self.transfer_section(
            "Awaiting Review",
            cards.len(),
            cards,
            "No files awaiting review.",
        )
    }

    fn in_progress_section(&self) -> Div {
        let cards = self
            .transfer_rail_items
            .iter()
            .filter(|item| item.is_in_progress())
            .map(Self::in_progress_card)
            .collect::<Vec<_>>();

        self.transfer_section("In Progress", cards.len(), cards, "No active transfers.")
    }

    fn completed_section(&self, cx: &mut Context<Self>) -> Div {
        let cards = self
            .transfer_rail_items
            .iter()
            .filter(|item| item.is_completed())
            .enumerate()
            .map(|(index, item)| self.completed_card(index, item, cx))
            .collect::<Vec<_>>();

        self.transfer_section("Completed", cards.len(), cards, "No completed transfers.")
    }

    fn transfer_section(
        &self,
        title: &str,
        count: usize,
        cards: Vec<Div>,
        empty_label: &str,
    ) -> Div {
        let content = if cards.is_empty() {
            vec![
                div()
                    .p(px(14.0))
                    .bg(theme::bg_activity())
                    .border_1()
                    .border_color(theme::border_soft())
                    .rounded(px(18.0))
                    .text_size(px(12.0))
                    .text_color(theme::fg_muted())
                    .child(empty_label.to_string()),
            ]
        } else {
            cards
        };

        div()
            .v_flex()
            .gap(px(12.0))
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .justify_between()
                    .gap(px(12.0))
                    .child(
                        div()
                            .text_size(px(14.0))
                            .font_semibold()
                            .text_color(theme::fg_primary())
                            .child(title.to_string()),
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(theme::fg_muted())
                            .child(count.to_string()),
                    ),
            )
            .children(content)
    }

    fn awaiting_review_card(
        &self,
        index: usize,
        item: &TransferRailItem,
        cx: &mut Context<Self>,
    ) -> Div {
        let queued_at_label = match &item.status {
            TransferRailStatus::AwaitingReview { queued_at_label } => queued_at_label,
            _ => unreachable!("awaiting review section only renders awaiting review items"),
        };

        div()
            .v_flex()
            .gap(px(12.0))
            .p(px(14.0))
            .bg(theme::bg_rail_panel())
            .border_1()
            .border_color(theme::border_soft())
            .rounded(px(18.0))
            .child(Self::transfer_card_heading(
                item.file_name.as_str(),
                theme::accent_amber(),
            ))
            .child(Self::transfer_meta_row(
                item.source_device.as_str(),
                item.size_label.as_str(),
            ))
            .child(Self::transfer_detail_pair(
                "Queued",
                queued_at_label.as_str(),
            ))
            .child(
                Button::new(("transfer-queue-open", index))
                    .label("Open Transfers")
                    .primary()
                    .on_click(cx.listener(|this, _, _, cx| this.open_transfers(cx))),
            )
    }

    fn in_progress_card(item: &TransferRailItem) -> Div {
        let (progress, speed_label, started_at_label, elapsed_label, eta_label) = match &item.status
        {
            TransferRailStatus::InProgress {
                progress,
                speed_label,
                started_at_label,
                elapsed_label,
                eta_label,
            } => (
                *progress,
                speed_label.as_str(),
                started_at_label.as_str(),
                elapsed_label.as_str(),
                eta_label.as_str(),
            ),
            _ => unreachable!("in progress section only renders active transfer items"),
        };

        div()
            .v_flex()
            .gap(px(12.0))
            .p(px(14.0))
            .bg(theme::bg_rail_panel())
            .border_1()
            .border_color(theme::border_soft())
            .rounded(px(18.0))
            .child(Self::transfer_card_heading(
                item.file_name.as_str(),
                theme::accent_blue(),
            ))
            .child(Self::transfer_meta_row(
                item.source_device.as_str(),
                item.size_label.as_str(),
            ))
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .justify_between()
                    .gap(px(12.0))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(theme::fg_secondary())
                            .child(speed_label.to_string()),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .font_semibold()
                            .text_color(theme::accent_blue())
                            .child(format!("{:.0}%", progress * 100.0)),
                    ),
            )
            .child(
                Progress::new(format!("transfer-rail-progress-{}", item.id))
                    .value(progress * 100.0),
            )
            .child(
                div()
                    .grid()
                    .grid_cols(2)
                    .gap(px(10.0))
                    .child(Self::transfer_detail_pair("Started", started_at_label))
                    .child(Self::transfer_detail_pair("Elapsed", elapsed_label))
                    .child(Self::transfer_detail_pair("Remaining", eta_label))
                    .child(Self::transfer_detail_pair("Speed", speed_label)),
            )
    }

    fn completed_card(&self, index: usize, item: &TransferRailItem, cx: &mut Context<Self>) -> Div {
        let (completed_at_label, duration_label) = match &item.status {
            TransferRailStatus::Completed {
                completed_at_label,
                duration_label,
            } => (completed_at_label.as_str(), duration_label.as_str()),
            _ => unreachable!("completed section only renders completed items"),
        };

        div()
            .v_flex()
            .gap(px(12.0))
            .p(px(14.0))
            .bg(theme::bg_rail_panel())
            .border_1()
            .border_color(theme::border_soft())
            .rounded(px(18.0))
            .child(Self::transfer_card_heading(
                item.file_name.as_str(),
                theme::accent_green(),
            ))
            .child(Self::transfer_meta_row(
                item.source_device.as_str(),
                item.size_label.as_str(),
            ))
            .child(
                div()
                    .grid()
                    .grid_cols(2)
                    .gap(px(10.0))
                    .child(Self::transfer_detail_pair("Completed", completed_at_label))
                    .child(Self::transfer_detail_pair("Duration", duration_label)),
            )
            .child({
                let item_id = item.id.clone();

                Button::new(("dismiss-transfer-complete", index))
                    .label("Dismiss")
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.dismiss_completed_item(item_id.as_str(), cx);
                    }))
            })
    }

    fn transfer_card_heading(file_name: &str, accent: gpui::Hsla) -> Div {
        div()
            .v_flex()
            .gap(px(8.0))
            .child(div().h(px(2.0)).w_full().bg(accent).rounded(px(999.0)))
            .child(
                div()
                    .text_size(px(13.0))
                    .font_semibold()
                    .text_color(theme::fg_primary())
                    .line_clamp(2)
                    .text_ellipsis()
                    .child(file_name.to_string()),
            )
    }

    fn transfer_meta_row(source_device: &str, size_label: &str) -> Div {
        div()
            .text_size(px(11.0))
            .text_color(theme::fg_muted())
            .truncate()
            .child(format!("{} · {}", source_device, size_label))
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
                    .text_size(px(12.0))
                    .text_color(theme::fg_secondary())
                    .child(value.to_string()),
            )
    }
}
