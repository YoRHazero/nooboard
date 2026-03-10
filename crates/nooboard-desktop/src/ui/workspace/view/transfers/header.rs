use gpui::{
    AnyElement, Context, Div, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, div, px,
};
use gpui_component::StyledExt;
use gpui_component::scroll::ScrollableElement;

use crate::ui::theme;

use super::WorkspaceView;
use super::components::{
    transfer_metric_chip, transfer_target_chip, transfers_empty_notice, transfers_panel_header,
    transfers_panel_shell,
};
use super::snapshot::TransfersSnapshot;

impl WorkspaceView {
    pub(super) fn transfers_header(&self, snapshot: &TransfersSnapshot) -> Div {
        transfers_panel_shell().child(
            div()
                .h_flex()
                .items_center()
                .justify_between()
                .gap(px(14.0))
                .child(transfers_panel_header(
                    "Transfers",
                    "App-backed staging, routing, and transfer activity.",
                ))
                .child(
                    div()
                        .h_flex()
                        .gap(px(8.0))
                        .child(transfer_metric_chip(
                            "Staged",
                            self.transfers_page_state.staged_file_count().to_string(),
                            theme::accent_blue(),
                        ))
                        .child(transfer_metric_chip(
                            "Awaiting",
                            snapshot.metrics.awaiting.to_string(),
                            theme::accent_amber(),
                        ))
                        .child(transfer_metric_chip(
                            "Active",
                            snapshot.metrics.active.to_string(),
                            theme::accent_cyan(),
                        ))
                        .child(transfer_metric_chip(
                            "Completed",
                            snapshot.metrics.completed.to_string(),
                            theme::accent_green(),
                        )),
                ),
        )
    }

    pub(super) fn transfers_target_panel(
        &self,
        snapshot: &TransfersSnapshot,
        cx: &mut Context<Self>,
    ) -> Div {
        let target_chips = snapshot
            .target_peers
            .iter()
            .map(|target| {
                let noob_id = target.noob_id.clone();
                transfer_target_chip(
                    &target.device_id,
                    &target.noob_id,
                    target.selected,
                    theme::accent_cyan(),
                )
                .id(format!("transfer-target-chip-{}", target.noob_id))
                .cursor_pointer()
                .hover(|this| {
                    this.bg(theme::bg_panel_alt())
                        .border_color(theme::border_strong())
                })
                .active(|this| this.bg(theme::bg_panel()))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.toggle_transfer_target(&noob_id, cx);
                }))
                .into_any_element()
            })
            .collect::<Vec<AnyElement>>();

        transfers_panel_shell()
            .child(transfers_panel_header(
                "Routing",
                format!(
                    "{} connected peer{}",
                    snapshot.target_peers.len(),
                    if snapshot.target_peers.len() == 1 {
                        ""
                    } else {
                        "s"
                    }
                ),
            ))
            .child(
                div()
                    .v_flex()
                    .gap(px(10.0))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .font_semibold()
                            .text_color(theme::fg_secondary())
                            .child("Connected targets"),
                    )
                    .child(
                        div().w_full().overflow_x_scrollbar().child(
                            div()
                                .h_flex()
                                .gap(px(10.0))
                                .children(if target_chips.is_empty() {
                                    vec![
                                        transfers_empty_notice(
                                            "No connected peers available for transfer.",
                                        )
                                        .into_any_element(),
                                    ]
                                } else {
                                    target_chips
                                }),
                        ),
                    ),
            )
            .child(div().h(px(1.0)).w_full().bg(theme::border_soft()))
            .child(
                div()
                    .v_flex()
                    .gap(px(10.0))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .font_semibold()
                            .text_color(theme::fg_secondary())
                            .child("Download directory"),
                    )
                    .child(
                        div()
                            .id("transfer-download-dir")
                            .w_full()
                            .min_w(px(0.0))
                            .cursor_pointer()
                            .px(px(14.0))
                            .py(px(12.0))
                            .bg(theme::bg_console())
                            .border_1()
                            .border_color(theme::border_soft())
                            .rounded(px(16.0))
                            .hover(|this| {
                                this.bg(theme::bg_panel_alt())
                                    .border_color(theme::border_strong())
                            })
                            .active(|this| this.bg(theme::bg_panel()))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.open_transfer_settings(cx);
                            }))
                            .child(
                                div()
                                    .v_flex()
                                    .gap(px(6.0))
                                    .min_w(px(0.0))
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .font_semibold()
                                            .text_color(theme::fg_primary())
                                            .line_clamp(1)
                                            .text_ellipsis()
                                            .child(snapshot.download_dir_label.clone()),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11.0))
                                            .text_color(theme::fg_muted())
                                            .child("Managed in Settings"),
                                    ),
                            ),
                    ),
            )
    }
}
