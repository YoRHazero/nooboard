use std::path::PathBuf;

use gpui::{
    Context, Corner, Div, InteractiveElement, ParentElement, StatefulInteractiveElement, Styled,
    div, prelude::FluentBuilder as _, px,
};
use gpui_component::button::{Button, ButtonCustomVariant, ButtonVariants};
use gpui_component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_component::progress::Progress;
use gpui_component::{Sizable, StyledExt};

use crate::state::{TransferItem, TransferStatus};
use crate::ui::theme;

use super::WorkspaceView;

impl WorkspaceView {
    pub(super) fn transfers_download_panel(&self, cx: &mut Context<Self>) -> Div {
        div()
            .v_flex()
            .gap(px(16.0))
            .p(px(18.0))
            .bg(theme::bg_panel())
            .border_1()
            .border_color(theme::border_base())
            .rounded(px(24.0))
            .shadow_xs()
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .justify_between()
                    .gap(px(12.0))
                    .child(
                        div()
                            .text_size(px(18.0))
                            .font_semibold()
                            .text_color(theme::fg_primary())
                            .child("Incoming Downloads"),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(theme::fg_muted())
                            .child(format!("{} total", self.transfer_items.len())),
                    ),
            )
            .child(self.download_global_folder_panel(cx))
            .child(self.download_awaiting_section(cx))
            .child(self.download_progress_section(cx))
            .child(self.download_complete_section(cx))
    }

    fn download_global_folder_panel(&self, cx: &mut Context<Self>) -> Div {
        let folder_label = self
            .transfers_page_state
            .global_folder
            .display()
            .to_string();
        let presets = self.transfer_folder_presets();
        let view = cx.entity().downgrade();

        div()
            .v_flex()
            .gap(px(8.0))
            .child(
                div()
                    .text_size(px(12.0))
                    .font_semibold()
                    .text_color(theme::fg_secondary())
                    .child("Global download folder"),
            )
            .child(
                div()
                    .id("download-global-folder-capsule")
                    .w_full()
                    .min_w(px(0.0))
                    .px(px(12.0))
                    .py(px(10.0))
                    .bg(theme::bg_console())
                    .border_1()
                    .border_color(theme::border_soft())
                    .rounded(px(14.0))
                    .cursor_pointer()
                    .hover(|this| {
                        this.bg(theme::bg_panel_alt())
                            .border_color(theme::border_strong())
                    })
                    .active(|this| this.bg(theme::bg_panel()))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.pick_transfer_global_folder(window, cx);
                    }))
                    .child(
                        div()
                            .w_full()
                            .min_w(px(0.0))
                            .text_size(px(12.0))
                            .text_color(theme::fg_secondary())
                            .line_clamp(1)
                            .text_ellipsis()
                            .child(folder_label),
                    ),
            )
            .child(
                div().h_flex().child(
                    Button::new("download-folder-presets")
                        .small()
                        .outline()
                        .label("Presets")
                        .dropdown_caret(true)
                        .dropdown_menu_with_anchor(Corner::TopRight, move |menu, _, _| {
                            presets.iter().fold(menu, |menu, (label, path)| {
                                let label = label.clone();
                                let path = path.clone();
                                let view = view.clone();
                                menu.item(PopupMenuItem::new(label).on_click(move |_, _, cx| {
                                    let _ = view.update(cx, |this, cx| {
                                        this.set_transfer_global_folder(path.clone(), cx);
                                    });
                                }))
                            })
                        }),
                ),
            )
            .child(div().h(px(1.0)).w_full().bg(theme::border_soft()))
    }

    fn download_awaiting_section(&self, cx: &mut Context<Self>) -> Div {
        let cards = self
            .transfer_items
            .iter()
            .filter(|item| matches!(item.status, TransferStatus::AwaitingReview { .. }))
            .map(|item| self.download_awaiting_card(item, cx))
            .collect::<Vec<_>>();

        self.download_section("Awaiting Review", cards, "No files awaiting review.")
    }

    fn download_progress_section(&self, cx: &mut Context<Self>) -> Div {
        let cards = self
            .transfer_items
            .iter()
            .filter(|item| matches!(item.status, TransferStatus::Progress { .. }))
            .map(|item| self.download_progress_card(item, cx))
            .collect::<Vec<_>>();

        self.download_section("Progress", cards, "No active download transfers.")
    }

    fn download_complete_section(&self, cx: &mut Context<Self>) -> Div {
        let cards = self
            .transfer_items
            .iter()
            .filter(|item| matches!(item.status, TransferStatus::Complete { .. }))
            .map(|item| self.download_complete_card(item, cx))
            .collect::<Vec<_>>();

        self.download_section("Complete", cards, "No completed downloads.")
    }

    fn download_section(&self, title: &str, cards: Vec<Div>, empty_label: &str) -> Div {
        let count = cards.len();

        div()
            .v_flex()
            .gap(px(10.0))
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .justify_between()
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
            .children(if cards.is_empty() {
                vec![
                    div()
                        .p(px(12.0))
                        .bg(theme::bg_activity())
                        .border_1()
                        .border_color(theme::border_soft())
                        .rounded(px(16.0))
                        .text_size(px(12.0))
                        .text_color(theme::fg_muted())
                        .child(empty_label.to_string()),
                ]
            } else {
                cards
            })
    }

    fn download_awaiting_card(&self, item: &TransferItem, cx: &mut Context<Self>) -> Div {
        let queued_at_label = match &item.status {
            TransferStatus::AwaitingReview { queued_at_label } => queued_at_label.as_str(),
            _ => "",
        };
        let item_id = item.id.clone();
        let accept_id = item.id.clone();
        let reject_id = item.id.clone();

        div()
            .v_flex()
            .gap(px(10.0))
            .p(px(14.0))
            .bg(theme::bg_rail_panel())
            .border_1()
            .border_color(theme::border_soft())
            .rounded(px(18.0))
            .child(self.download_card_title(item.file_name.as_str(), theme::accent_amber()))
            .child(self.download_card_meta(item.source_device.as_str(), item.size_label.as_str()))
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(theme::fg_muted())
                    .child(format!("Queued at {}", queued_at_label)),
            )
            .child(
                div()
                    .h_flex()
                    .gap(px(8.0))
                    .child(
                        self.download_action_button(
                            format!("download-accept-{}", item_id),
                            "Accept",
                            theme::accent_green(),
                            cx,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.accept_download_transfer(accept_id.as_str(), cx);
                        })),
                    )
                    .child(
                        self.download_action_button(
                            format!("download-reject-{}", item.id),
                            "Reject",
                            theme::accent_rose(),
                            cx,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.reject_download_transfer(reject_id.as_str(), cx);
                        })),
                    ),
            )
    }

    fn download_progress_card(&self, item: &TransferItem, cx: &mut Context<Self>) -> Div {
        let (progress, speed_label, elapsed_label, eta_label) = match &item.status {
            TransferStatus::Progress {
                progress,
                speed_label,
                elapsed_label,
                eta_label,
                ..
            } => (
                *progress,
                speed_label.as_str(),
                elapsed_label.as_str(),
                eta_label.as_str(),
            ),
            _ => (0.0, "", "", ""),
        };
        let cancel_id = item.id.clone();

        div()
            .v_flex()
            .gap(px(10.0))
            .p(px(14.0))
            .bg(theme::bg_rail_panel())
            .border_1()
            .border_color(theme::border_soft())
            .rounded(px(18.0))
            .child(self.download_card_title(item.file_name.as_str(), theme::accent_blue()))
            .child(self.download_card_meta(item.source_device.as_str(), item.size_label.as_str()))
            .child(
                div()
                    .h_flex()
                    .justify_between()
                    .items_center()
                    .gap(px(10.0))
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
            .child(Progress::new(format!("download-progress-{}", item.id)).value(progress * 100.0))
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(theme::fg_muted())
                    .child(format!("{} · {}", elapsed_label, eta_label)),
            )
            .child(
                div().h_flex().child(
                    self.download_action_button(
                        format!("download-cancel-{}", item.id),
                        "Cancel",
                        theme::accent_rose(),
                        cx,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.cancel_download_transfer(cancel_id.as_str(), cx);
                    })),
                ),
            )
    }

    fn download_complete_card(&self, item: &TransferItem, cx: &mut Context<Self>) -> Div {
        let (completed_at_label, duration_label) = match &item.status {
            TransferStatus::Complete {
                completed_at_label,
                duration_label,
            } => (completed_at_label.as_str(), duration_label.as_str()),
            _ => ("", ""),
        };
        let move_to = self
            .transfers_page_state
            .moved_download_paths
            .get(&item.id)
            .map(|path| path.display().to_string());
        let got_it_id = item.id.clone();
        let move_id = item.id.clone();

        div()
            .v_flex()
            .gap(px(10.0))
            .p(px(14.0))
            .bg(theme::bg_rail_panel())
            .border_1()
            .border_color(theme::border_soft())
            .rounded(px(18.0))
            .child(self.download_card_title(item.file_name.as_str(), theme::accent_green()))
            .child(self.download_card_meta(item.source_device.as_str(), item.size_label.as_str()))
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(theme::fg_muted())
                    .child(format!(
                        "Completed {} · {}",
                        completed_at_label, duration_label
                    )),
            )
            .when_some(move_to, |this, path| {
                this.child(
                    div()
                        .text_size(px(11.0))
                        .text_color(theme::accent_cyan())
                        .line_clamp(1)
                        .text_ellipsis()
                        .child(format!("Move to {}", path)),
                )
            })
            .child(
                div()
                    .h_flex()
                    .gap(px(8.0))
                    .child(
                        self.download_action_button(
                            format!("download-got-it-{}", item.id),
                            "Got it",
                            theme::accent_green(),
                            cx,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.got_it_download_transfer(got_it_id.as_str(), cx);
                        })),
                    )
                    .child(
                        self.download_action_button(
                            format!("download-move-to-{}", item.id),
                            "Move to",
                            theme::accent_cyan(),
                            cx,
                        )
                        .on_click(cx.listener(
                            move |this, _, window, cx| {
                                this.move_complete_download_transfer(move_id.clone(), window, cx);
                            },
                        )),
                    ),
            )
    }

    fn download_card_title(&self, file_name: &str, accent: gpui::Hsla) -> Div {
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

    fn download_card_meta(&self, source_device: &str, size_label: &str) -> Div {
        div()
            .text_size(px(11.0))
            .text_color(theme::fg_muted())
            .truncate()
            .child(format!("{} · {}", source_device, size_label))
    }

    fn download_action_button(
        &self,
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

    fn transfer_folder_presets(&self) -> Vec<(String, PathBuf)> {
        let mut presets = vec![(
            "Workspace downloads".to_string(),
            PathBuf::from(".dev-data/downloads"),
        )];

        if let Ok(home) = std::env::var("HOME") {
            presets.push((
                "Home downloads".to_string(),
                PathBuf::from(home).join("Downloads"),
            ));
        }
        presets.push(("Temporary folder".to_string(), PathBuf::from("/tmp")));

        presets
    }
}
