use gpui::{
    Context, Div, ExternalPaths, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, div, px,
};
use gpui_component::{Disableable, StyledExt};

use crate::ui::theme;

use super::WorkspaceView;
use super::components::{
    transfer_action_button, transfers_card_shell, transfers_empty_notice, transfers_panel_header,
    transfers_panel_shell,
};
use super::page_state::{StagedFileSource, StagedTransferFile, TransfersSendState};

impl WorkspaceView {
    pub(super) fn transfers_upload_panel(&self, cx: &mut Context<Self>) -> Div {
        let staged_cards = self
            .transfers_page_state
            .staged_files
            .iter()
            .map(|item| self.staged_transfer_card(item, cx))
            .collect::<Vec<_>>();
        let can_send = !self.transfers_page_state.staged_files.is_empty()
            && self.transfers_page_state.selected_target_count() > 0
            && !self.transfers_page_state.send_in_flight();
        let send_label = match self.transfers_page_state.send_state {
            TransfersSendState::Idle => "Send staged files",
            TransfersSendState::Sending => "Sending...",
        };

        transfers_panel_shell()
            .child(transfers_panel_header(
                "Send Composer",
                format!(
                    "{} staged · {} selected target{}",
                    self.transfers_page_state.staged_file_count(),
                    self.transfers_page_state.selected_target_count(),
                    if self.transfers_page_state.selected_target_count() == 1 {
                        ""
                    } else {
                        "s"
                    }
                ),
            ))
            .child(self.transfer_upload_drop_zone(cx))
            .child(
                div()
                    .v_flex()
                    .gap(px(10.0))
                    .children(if staged_cards.is_empty() {
                        vec![
                            transfers_empty_notice("No local files staged yet.").into_any_element(),
                        ]
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
                            .child(self.transfers_page_state.feedback.clone().unwrap_or_else(
                                || {
                                    "Stage local files, then hand off to the app transfer service."
                                        .to_string()
                                },
                            )),
                    )
                    .child(
                        transfer_action_button(
                            "transfer-submit-staged",
                            send_label,
                            theme::accent_cyan(),
                            cx,
                        )
                        .disabled(!can_send)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.submit_staged_transfers(cx);
                        })),
                    ),
            )
    }

    fn transfer_upload_drop_zone(&self, cx: &mut Context<Self>) -> impl IntoElement {
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
                this.queue_upload_paths(paths.paths().to_vec(), StagedFileSource::Dropped, cx);
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
                    .child(
                        "Only staged files live here. Active transfer progress comes from the app.",
                    ),
            )
            .child(div().h_flex().gap(px(8.0)).child(self.transfer_browse_files_chip(cx)))
    }

    fn transfer_browse_files_chip(&self, cx: &mut Context<Self>) -> impl IntoElement {
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
            }))
    }

    fn staged_transfer_card(&self, item: &StagedTransferFile, cx: &mut Context<Self>) -> Div {
        let remove_id = item.id.clone();

        transfers_card_shell()
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
                    .child(format!(
                        "{} · {} B · modified {}",
                        item.size_label, item.size_bytes, item.modified_at_label
                    )),
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
                    transfer_action_button(
                        format!("dismiss-staged-transfer-{}", item.id),
                        "Remove",
                        theme::accent_rose(),
                        cx,
                    )
                    .disabled(self.transfers_page_state.send_in_flight())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.dismiss_staged_file(&remove_id, cx);
                    })),
                ),
            )
    }
}
