use gpui::{Context, Div, ElementId, Hsla, ParentElement, Styled, div, px};
use gpui_component::button::{Button, ButtonCustomVariant, ButtonVariants};
use gpui_component::progress::Progress;
use gpui_component::{Sizable, StyledExt};

use crate::state::{TransferItem, TransferStatus};
use crate::ui::theme;

use super::WorkspaceView;

impl WorkspaceView {
    pub(super) fn transfer_sections(&self, cx: &mut Context<Self>) -> Div {
        div()
            .v_flex()
            .gap(px(18.0))
            .child(self.awaiting_review_section(cx))
            .child(self.progress_section())
            .child(self.complete_section(cx))
    }

    fn awaiting_review_section(&self, cx: &mut Context<Self>) -> Div {
        let cards = self
            .transfer_items
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

    fn progress_section(&self) -> Div {
        let cards = self
            .transfer_items
            .iter()
            .filter(|item| item.is_progress())
            .map(Self::progress_card)
            .collect::<Vec<_>>();

        self.transfer_section("Progress", cards.len(), cards, "No files in progress.")
    }

    fn complete_section(&self, cx: &mut Context<Self>) -> Div {
        let cards = self
            .transfer_items
            .iter()
            .filter(|item| item.is_complete())
            .enumerate()
            .map(|(index, item)| self.complete_card(index, item, cx))
            .collect::<Vec<_>>();

        self.transfer_section("Complete", cards.len(), cards, "No completed files.")
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
        item: &TransferItem,
        cx: &mut Context<Self>,
    ) -> Div {
        let queued_at_label = match &item.status {
            TransferStatus::AwaitingReview { queued_at_label } => queued_at_label,
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
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .justify_between()
                    .gap(px(10.0))
                    .child(Self::transfer_inline_note("Queued", queued_at_label.as_str()).flex_1())
                    .child(
                        Self::transfer_action_button(
                            ("transfer-queue-open", index),
                            "Check",
                            theme::accent_amber(),
                            cx,
                        )
                        .on_click(cx.listener(|this, _, _, cx| this.open_transfers(cx))),
                    ),
            )
    }

    fn progress_card(item: &TransferItem) -> Div {
        let (progress, speed_label, started_at_label, elapsed_label, eta_label) = match &item.status
        {
            TransferStatus::Progress {
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

    fn complete_card(&self, index: usize, item: &TransferItem, cx: &mut Context<Self>) -> Div {
        let (completed_at_label, duration_label) = match &item.status {
            TransferStatus::Complete {
                completed_at_label,
                duration_label,
            } => (completed_at_label.as_str(), duration_label.as_str()),
            _ => unreachable!("complete section only renders complete items"),
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
            .child(Self::transfer_complete_meta_row(
                item.source_device.as_str(),
                item.size_label.as_str(),
                duration_label,
            ))
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .justify_between()
                    .gap(px(10.0))
                    .child(Self::transfer_inline_note("Complete", completed_at_label).flex_1())
                    .child({
                        let item_id = item.id.clone();

                        Self::transfer_action_button(
                            ("dismiss-transfer-complete", index),
                            "Got it",
                            theme::accent_green(),
                            cx,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.dismiss_complete_item(item_id.as_str(), cx);
                        }))
                    }),
            )
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

    fn transfer_complete_meta_row(
        source_device: &str,
        size_label: &str,
        duration_label: &str,
    ) -> Div {
        div()
            .text_size(px(11.0))
            .text_color(theme::fg_muted())
            .truncate()
            .child(format!(
                "{} · {} · done in {}",
                source_device, size_label, duration_label
            ))
    }

    fn transfer_action_button(
        id: impl Into<ElementId>,
        label: &str,
        accent: Hsla,
        cx: &mut Context<Self>,
    ) -> Button {
        let variant = ButtonCustomVariant::new(cx)
            .color(accent.opacity(0.12))
            .foreground(theme::fg_primary())
            .hover(accent.opacity(0.2))
            .active(accent.opacity(0.28))
            .shadow(false);

        Button::new(id)
            .custom(variant)
            .small()
            .compact()
            .rounded(px(999.0))
            .border_1()
            .border_color(accent.opacity(0.24))
            .child(
                div()
                    .text_color(theme::fg_primary())
                    .font_semibold()
                    .child(label.to_string()),
            )
    }

    fn transfer_inline_note(label: &str, value: &str) -> Div {
        div()
            .h_flex()
            .items_center()
            .gap(px(6.0))
            .min_w(px(0.0))
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
                    .truncate()
                    .child(value.to_string()),
            )
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
