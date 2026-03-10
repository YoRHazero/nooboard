use gpui::{Context, Div, ParentElement, Styled, div, px};
use gpui_component::button::{Button, ButtonCustomVariant, ButtonVariants};
use gpui_component::progress::Progress;
use gpui_component::{Sizable, StyledExt};

use crate::ui::theme;

use super::super::transfers::snapshot::{
    ActiveTransferCardSnapshot, CompletedTransferCardSnapshot, IncomingTransferCardSnapshot,
    TransferVisualAccent, TransfersSnapshot,
};
use super::WorkspaceView;

impl WorkspaceView {
    pub(super) fn transfer_sections(
        &self,
        snapshot: &TransfersSnapshot,
        cx: &mut Context<Self>,
    ) -> Div {
        let active_cards = snapshot
            .active_uploads
            .iter()
            .chain(snapshot.active_downloads.iter())
            .collect::<Vec<_>>();
        let completed_cards = snapshot
            .completed_uploads
            .iter()
            .chain(snapshot.completed_downloads.iter())
            .collect::<Vec<_>>();

        div()
            .v_flex()
            .gap(px(18.0))
            .child(self.awaiting_review_section(&snapshot.incoming_pending, cx))
            .child(self.progress_section(&active_cards))
            .child(self.complete_section(&completed_cards, cx))
    }

    fn awaiting_review_section(
        &self,
        items: &[IncomingTransferCardSnapshot],
        cx: &mut Context<Self>,
    ) -> Div {
        let cards = items
            .iter()
            .enumerate()
            .map(|(index, item)| self.awaiting_review_card(index, item, cx))
            .collect::<Vec<_>>();

        self.transfer_section("Awaiting", cards.len(), cards, "No files awaiting review.")
    }

    fn progress_section(&self, items: &[&ActiveTransferCardSnapshot]) -> Div {
        let cards = items
            .iter()
            .map(|item| Self::progress_card(item))
            .collect::<Vec<_>>();

        self.transfer_section("Active", cards.len(), cards, "No files in progress.")
    }

    fn complete_section(
        &self,
        items: &[&CompletedTransferCardSnapshot],
        cx: &mut Context<Self>,
    ) -> Div {
        let cards = items
            .iter()
            .enumerate()
            .map(|(index, item)| self.complete_card(index, item, cx))
            .collect::<Vec<_>>();

        self.transfer_section("Completed", cards.len(), cards, "No completed files.")
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
        item: &IncomingTransferCardSnapshot,
        cx: &mut Context<Self>,
    ) -> Div {
        div()
            .v_flex()
            .gap(px(12.0))
            .p(px(14.0))
            .bg(theme::bg_rail_panel())
            .border_1()
            .border_color(theme::border_soft())
            .rounded(px(18.0))
            .child(Self::transfer_card_heading(
                &item.file_name,
                theme::accent_amber(),
            ))
            .child(Self::transfer_meta_row(
                &item.peer_device_id,
                &item.file_size_label,
            ))
            .child(Self::transfer_inline_note("Offered", &item.offered_at_label).flex_1())
            .child(
                div().h_flex().justify_end().child(
                    Self::transfer_action_button(
                        ("transfer-queue-open", index),
                        "Review",
                        theme::accent_amber(),
                        cx,
                    )
                    .on_click(cx.listener(|this, _, _, cx| this.open_transfers(cx))),
                ),
            )
    }

    fn progress_card(item: &ActiveTransferCardSnapshot) -> Div {
        let accent = transfer_accent_color(item.state_accent);
        let speed_label = item
            .speed_label
            .clone()
            .unwrap_or_else(|| fallback_estimate_label(&item.state_label));

        div()
            .v_flex()
            .gap(px(12.0))
            .p(px(14.0))
            .bg(theme::bg_rail_panel())
            .border_1()
            .border_color(theme::border_soft())
            .rounded(px(18.0))
            .child(Self::transfer_card_heading(&item.file_name, accent))
            .child(Self::transfer_meta_row(
                &item.peer_device_id,
                &item.file_size_label,
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
                Progress::new(format!("transfer-rail-progress-{}", item.transfer_id))
                    .value(item.progress_fraction * 100.0),
            )
            .child(
                div()
                    .grid()
                    .grid_cols(2)
                    .gap(px(10.0))
                    .child(Self::transfer_detail_pair("State", item.state_label))
                    .child(Self::transfer_detail_pair("Speed", &speed_label))
                    .child(Self::transfer_detail_pair(
                        "Started",
                        &item.started_at_label,
                    ))
                    .child(Self::transfer_detail_pair(
                        "Updated",
                        &item.updated_at_label,
                    )),
            )
    }

    fn complete_card(
        &self,
        index: usize,
        item: &CompletedTransferCardSnapshot,
        cx: &mut Context<Self>,
    ) -> Div {
        let accent = transfer_accent_color(item.outcome_accent);

        div()
            .v_flex()
            .gap(px(12.0))
            .p(px(14.0))
            .bg(theme::bg_rail_panel())
            .border_1()
            .border_color(theme::border_soft())
            .rounded(px(18.0))
            .child(Self::transfer_card_heading(&item.file_name, accent))
            .child(Self::transfer_complete_meta_row(
                &item.peer_device_id,
                &item.file_size_label,
                item.duration_label.as_deref().unwrap_or("completed"),
            ))
            .child(Self::transfer_inline_note(item.outcome_label, &item.finished_at_label).flex_1())
            .child(
                div().h_flex().justify_end().child(
                    Self::transfer_action_button(
                        ("open-transfer-complete", index),
                        "View",
                        accent,
                        cx,
                    )
                    .on_click(cx.listener(|this, _, _, cx| this.open_transfers(cx))),
                ),
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
                "{} · {} · {}",
                source_device, size_label, duration_label
            ))
    }

    fn transfer_action_button(
        id: impl Into<gpui::ElementId>,
        label: &str,
        accent: gpui::Hsla,
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
                    .text_size(px(11.0))
                    .text_color(theme::fg_secondary())
                    .line_clamp(1)
                    .text_ellipsis()
                    .child(value.to_string()),
            )
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

fn fallback_estimate_label(state_label: &str) -> String {
    match state_label {
        "In Progress" => "Estimating".to_string(),
        "Cancelling" => "Stopping".to_string(),
        _ => "Pending".to_string(),
    }
}
