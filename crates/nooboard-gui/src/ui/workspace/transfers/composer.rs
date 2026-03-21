use gpui::{
    Context, ExternalPaths, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, div, px,
};
use gpui_component::{Disableable, StyledExt};

use crate::ui::theme;

use super::super::WorkspaceView;
use super::StagedTransferFile;

impl WorkspaceView {
    pub(in crate::ui::workspace::transfers) fn transfers_upload_panel(
        &self,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let staged_cards = self
            .transfers
            .staged_files()
            .iter()
            .cloned()
            .map(|item| self.staged_transfer_card(item, cx))
            .collect::<Vec<_>>();

        self.transfers_panel_shell(
            "Send Files",
            format!(
                "{} selected · {} device(s) chosen",
                self.transfers.staged_files().len(),
                self.transfers.selected_session_ids().len()
            ),
        )
        .child(self.transfer_upload_drop_zone(cx))
        .child(
            div()
                .v_flex()
                .gap(px(10.0))
                .children(if staged_cards.is_empty() {
                    vec![self
                        .transfers_empty_notice("No local files staged yet.")
                        .into_any_element()]
                } else {
                    staged_cards
                        .into_iter()
                        .map(|card| card.into_any_element())
                        .collect()
                }),
        )
        .child(
            div()
                .h_flex()
                .items_center()
                .justify_between()
                .gap(px(12.0))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .text_size(px(11.0))
                        .text_color(theme::fg_muted())
                        .line_clamp(2)
                        .text_ellipsis()
                        .child(self.transfers.feedback().cloned().unwrap_or_else(|| {
                            "Add files here, choose one or more connected devices, then send."
                                .to_string()
                        })),
                )
                .child(
                    self.transfer_action_button(
                        "transfer-submit-staged",
                        if self.transfers.send_in_flight() {
                            "Sending..."
                        } else {
                            "Send Files"
                        },
                        theme::accent_cyan(),
                        cx,
                    )
                    .disabled(self.transfers.send_in_flight())
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.request_transfer_send(cx);
                    })),
                ),
        )
    }

    pub(in crate::ui::workspace::transfers) fn transfer_upload_drop_zone(
        &self,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        div()
            .id("transfer-upload-drop-zone")
            .v_flex()
            .gap(px(10.0))
            .p(px(16.0))
            .bg(theme::bg_console())
            .border_1()
            .border_color(theme::border_soft())
            .rounded(px(18.0))
            .drag_over::<ExternalPaths>(|style, _, _, _| {
                style
                    .bg(theme::bg_panel_highlight())
                    .border_color(theme::accent_cyan().opacity(0.55))
            })
            .on_drop(cx.listener(|this, paths: &ExternalPaths, _, cx| {
                this.queue_transfer_drop_paths(paths.paths().to_vec(), cx);
            }))
            .child(
                div()
                    .text_size(px(14.0))
                    .font_semibold()
                    .text_color(theme::fg_primary())
                    .child("Drop files here"),
            )
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(theme::fg_muted())
                    .child("Files added here are ready to send. Progress appears below once sending starts."),
            )
            .child(
                div()
                    .h_flex()
                    .gap(px(8.0))
                    .child(
                        div()
                            .id("transfer-upload-browse")
                            .cursor_pointer()
                            .px(px(10.0))
                            .py(px(7.0))
                            .rounded(px(999.0))
                            .bg(theme::accent_cyan().opacity(0.12))
                            .border_1()
                            .border_color(theme::accent_cyan().opacity(0.24))
                            .text_size(px(10.0))
                            .font_semibold()
                            .text_color(theme::accent_cyan())
                            .hover(|this| {
                                this.bg(theme::accent_cyan().opacity(0.18))
                                    .border_color(theme::accent_cyan().opacity(0.32))
                            })
                            .active(|this| this.bg(theme::accent_cyan().opacity(0.24)))
                            .child("Browse Files")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.pick_transfer_upload_files(window, cx);
                            })),
                    ),
            )
    }

    pub(in crate::ui::workspace::transfers) fn staged_transfer_card(
        &self,
        item: StagedTransferFile,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let remove_id = item.id.clone();

        self.transfers_card_shell()
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .justify_between()
                    .gap(px(10.0))
                    .child(
                        div()
                            .text_size(px(13.0))
                            .font_semibold()
                            .text_color(theme::fg_primary())
                            .line_clamp(1)
                            .text_ellipsis()
                            .child(item.file_name.clone()),
                    )
                    .child(
                        div()
                            .px(px(10.0))
                            .py(px(6.0))
                            .rounded(px(999.0))
                            .bg(theme::accent_blue().opacity(0.12))
                            .border_1()
                            .border_color(theme::accent_blue().opacity(0.24))
                            .text_size(px(10.0))
                            .font_semibold()
                            .text_color(theme::accent_blue())
                            .child(item.source.label()),
                    ),
            )
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(theme::fg_muted())
                    .line_clamp(1)
                    .text_ellipsis()
                    .child(format!("{} · Modified {}", item.size_label, item.modified_at_label)),
            )
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(theme::fg_secondary())
                    .line_clamp(1)
                    .text_ellipsis()
                    .child(item.file_path.display().to_string()),
            )
            .child(
                div().h_flex().justify_end().child(
                    self.transfer_action_button(
                        format!("dismiss-staged-transfer-{}", item.id),
                        "Remove",
                        theme::accent_rose(),
                        cx,
                    )
                    .disabled(self.transfers.send_in_flight())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.request_transfer_remove_staged(&remove_id, cx);
                    })),
                ),
            )
    }
}
