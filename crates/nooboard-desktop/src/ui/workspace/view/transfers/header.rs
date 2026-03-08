use gpui::{
    AnyElement, Context, Div, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, div, px,
};
use gpui_component::StyledExt;
use gpui_component::scroll::ScrollableElement;

use crate::state::{ClipboardTarget, ClipboardTargetStatus};
use crate::ui::theme;

use super::components::{
    transfer_metric_chip, transfer_target_chip, transfers_panel_header, transfers_panel_shell,
};
use super::WorkspaceView;

impl WorkspaceView {
    pub(super) fn transfers_header(&self) -> Div {
        transfers_panel_shell()
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .justify_between()
                    .gap(px(14.0))
                    .child(transfers_panel_header(
                        "Transfers",
                        "Local send queue and incoming file lanes.",
                    ))
                    .child(
                        div()
                            .h_flex()
                            .gap(px(8.0))
                            .child(transfer_metric_chip(
                                "Uploads",
                                self.transfers_page_state.uploads.len().to_string(),
                                theme::accent_blue(),
                            ))
                            .child(transfer_metric_chip(
                                "Awaiting",
                                self.awaiting_review_count().to_string(),
                                theme::accent_amber(),
                            ))
                            .child(transfer_metric_chip(
                                "Progress",
                                self.progress_count().to_string(),
                                theme::accent_cyan(),
                            )),
                    ),
            )
    }

    pub(super) fn transfers_target_panel(&self, cx: &mut Context<Self>) -> Div {
        self.transfer_targets_card(cx)
    }

    fn transfer_targets_card(&self, cx: &mut Context<Self>) -> Div {
        let target_chips = self
            .state
            .app
            .clipboard
            .targets
            .iter()
            .map(|target| self.transfer_target_chip(target, cx))
            .collect::<Vec<_>>();

        transfers_panel_shell()
            .child(transfers_panel_header("Targets", ""))
            .child(div().h(px(1.0)).w_full().bg(theme::border_soft()))
            .child(
                div()
                    .w_full()
                    .overflow_x_scrollbar()
                    .child(div().h_flex().gap(px(10.0)).children(target_chips)),
            )
    }

    fn transfer_target_chip(&self, target: &ClipboardTarget, cx: &mut Context<Self>) -> AnyElement {
        let connected = target.status == ClipboardTargetStatus::Connected;
        let selected = self.transfer_target_is_selected(&target.noob_id);
        let accent = if connected {
            theme::accent_cyan()
        } else {
            theme::fg_muted()
        };
        let noob_id = target.noob_id.clone();

        let mut chip = transfer_target_chip(target.device_id.clone(), connected, selected, accent)
            .id(format!("transfer-target-chip-{}", target.noob_id));

        if connected {
            chip = chip
                .cursor_pointer()
                .hover(|this| {
                    this.bg(theme::bg_panel_alt())
                        .border_color(theme::border_strong())
                })
                .active(|this| this.bg(theme::bg_panel()))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.toggle_transfer_target(&noob_id, cx);
                }));
        } else {
            chip = chip.opacity(0.72);
        }

        chip.into_any_element()
    }
}
