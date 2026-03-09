use gpui::{
    Context, Div, ExternalPaths, InteractiveElement, IntoElement, ParentElement, Styled, div,
    prelude::FluentBuilder as _, px,
};
use gpui_component::button::Button;
use gpui_component::progress::Progress;
use gpui_component::{Disableable, Sizable, StyledExt};

use crate::ui::theme;

use super::WorkspaceView;
use super::components::{
    local_upload_status_badge, transfer_action_button, transfers_card_shell,
    transfers_empty_notice, transfers_panel_header, transfers_panel_shell,
};
use super::page_state::{LocalUploadCard, LocalUploadStatus, UploadSource};

impl WorkspaceView {
    pub(super) fn transfers_upload_panel(&self, cx: &mut Context<Self>) -> Div {
        let upload_cards = self
            .transfers_page_state
            .uploads
            .iter()
            .map(|item| self.local_upload_card(item, cx))
            .collect::<Vec<_>>();

        transfers_panel_shell()
            .child(transfers_panel_header(
                "Local Upload Queue",
                format!("{} items", self.transfers_page_state.uploads.len()),
            ))
            .child(self.transfer_upload_drop_zone(cx))
            .child(
                div()
                    .v_flex()
                    .gap(px(10.0))
                    .children(if upload_cards.is_empty() {
                        vec![
                            transfers_empty_notice("No local files queued yet.").into_any_element(),
                        ]
                    } else {
                        upload_cards
                            .into_iter()
                            .map(|card| card.into_any_element())
                            .collect()
                    }),
            )
            .when_some(
                self.transfers_page_state.feedback.as_ref(),
                |this, message| {
                    this.child(
                        div()
                            .text_size(px(11.0))
                            .text_color(theme::fg_muted())
                            .line_clamp(1)
                            .text_ellipsis()
                            .child(message.clone()),
                    )
                },
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
                this.queue_upload_paths(paths.paths().to_vec(), UploadSource::Dropped, cx);
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
                    .child("Or browse local paths and stage them as sendable cards."),
            )
            .child(
                div().h_flex().gap(px(8.0)).child(
                    Button::new("transfer-upload-browse")
                        .small()
                        .outline()
                        .label("Browse Files")
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.pick_transfer_upload_files(window, cx);
                        })),
                ),
            )
    }

    fn local_upload_card(&self, item: &LocalUploadCard, cx: &mut Context<Self>) -> Div {
        let can_send = self.selected_transfer_target_count() > 0
            && matches!(
                item.status,
                LocalUploadStatus::Draft | LocalUploadStatus::Rejected { .. }
            );
        let item_id = item.id.clone();
        let send_id = item.id.clone();
        let dismiss_id = item.id.clone();

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
                    .child(local_upload_status_badge(&item.status)),
            )
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(theme::fg_muted())
                    .line_clamp(2)
                    .text_ellipsis()
                    .child(format!(
                        "{} ({} B) · {} · modified {} · {}",
                        item.size_label,
                        item.size_bytes,
                        item.file_path.display(),
                        item.modified_at_label,
                        item.source.label()
                    )),
            )
            .when(!item.sent_target_ids.is_empty(), |this| {
                this.child(
                    div()
                        .text_size(px(11.0))
                        .text_color(theme::fg_secondary())
                        .child(format!("Targets: {}", item.sent_target_ids.len())),
                )
            })
            .when_some(progress_value(&item.status), |this, value| {
                this.child(
                    Progress::new(format!("transfer-upload-progress-{}", item_id)).value(value),
                )
            })
            .when_some(status_detail(&item.status), |this, detail| {
                this.child(
                    div()
                        .text_size(px(11.0))
                        .text_color(theme::fg_secondary())
                        .line_clamp(1)
                        .text_ellipsis()
                        .child(detail),
                )
            })
            .child(
                div()
                    .h_flex()
                    .gap(px(8.0))
                    .child(
                        transfer_action_button(
                            format!("transfer-upload-send-{}", item.id),
                            "Send",
                            theme::accent_cyan(),
                            cx,
                        )
                        .disabled(!can_send)
                        .on_click(cx.listener(
                            move |this, _, window, cx| {
                                this.request_send_local_upload(send_id.as_str(), window, cx);
                            },
                        )),
                    )
                    .child(
                        transfer_action_button(
                            format!("transfer-upload-dismiss-{}", item.id),
                            "Dismiss",
                            theme::accent_rose(),
                            cx,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.dismiss_local_upload(dismiss_id.as_str(), cx);
                        })),
                    ),
            )
    }
}

fn progress_value(status: &LocalUploadStatus) -> Option<f32> {
    match status {
        LocalUploadStatus::Progress { progress, .. } => Some(progress * 100.0),
        _ => None,
    }
}

fn status_detail(status: &LocalUploadStatus) -> Option<String> {
    match status {
        LocalUploadStatus::Accepted {
            at_label,
            accepted_targets,
        } => Some(format!(
            "{} target{} accepted at {}",
            accepted_targets,
            if *accepted_targets == 1 { "" } else { "s" },
            at_label
        )),
        LocalUploadStatus::Rejected { at_label, reason } => {
            Some(format!("Rejected at {}: {}", at_label, reason))
        }
        LocalUploadStatus::Progress {
            speed_label,
            eta_label,
            ..
        } => Some(format!("{} · {}", speed_label, eta_label)),
        LocalUploadStatus::Complete { at_label } => Some(format!("Completed at {}", at_label)),
        LocalUploadStatus::Draft => None,
    }
}
