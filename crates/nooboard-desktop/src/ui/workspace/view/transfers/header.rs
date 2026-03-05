use gpui::{
    AnyElement, Context, Div, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, div, px,
};
use gpui_component::scroll::ScrollableElement;
use gpui_component::StyledExt;

use crate::state::{ClipboardTarget, ClipboardTargetStatus};
use crate::ui::theme;

use super::WorkspaceView;

impl WorkspaceView {
    pub(super) fn transfers_header(&self) -> Div {
        div()
            .h_flex()
            .items_center()
            .justify_between()
            .gap(px(14.0))
            .p(px(18.0))
            .bg(theme::bg_panel())
            .border_1()
            .border_color(theme::border_base())
            .rounded(px(24.0))
            .shadow_xs()
            .child(
                div()
                    .v_flex()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_size(px(22.0))
                            .font_semibold()
                            .text_color(theme::fg_primary())
                            .child("Transfers"),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(theme::fg_muted())
                            .child("Local send queue and incoming file lanes."),
                    ),
            )
            .child(
                div()
                    .h_flex()
                    .gap(px(8.0))
                    .child(self.transfer_metric_chip(
                        "Uploads",
                        self.transfers_page_state.uploads.len().to_string(),
                        theme::accent_blue(),
                    ))
                    .child(self.transfer_metric_chip(
                        "Awaiting",
                        self.awaiting_review_count().to_string(),
                        theme::accent_amber(),
                    ))
                    .child(self.transfer_metric_chip(
                        "Progress",
                        self.progress_count().to_string(),
                        theme::accent_cyan(),
                    )),
            )
    }

    fn transfer_metric_chip(&self, label: &str, value: String, accent: gpui::Hsla) -> Div {
        div()
            .h_flex()
            .items_center()
            .gap(px(8.0))
            .px(px(12.0))
            .py(px(9.0))
            .bg(theme::bg_console())
            .border_1()
            .border_color(accent.opacity(0.22))
            .rounded(px(16.0))
            .child(
                div()
                    .text_size(px(10.0))
                    .font_semibold()
                    .text_color(accent)
                    .child(label.to_string()),
            )
            .child(
                div()
                    .text_size(px(13.0))
                    .font_semibold()
                    .text_color(theme::fg_primary())
                    .child(value),
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

        div()
            .v_flex()
            .gap(px(12.0))
            .p(px(18.0))
            .bg(theme::bg_panel())
            .border_1()
            .border_color(theme::border_base())
            .rounded(px(22.0))
            .shadow_xs()
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .justify_between()
                    .gap(px(12.0))
                    .child(
                        div()
                            .text_size(px(15.0))
                            .font_semibold()
                            .text_color(theme::fg_primary())
                            .child("Targets"),
                    ),
            )
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
        let selected = self.transfer_target_is_selected(&target.node_id);
        let accent = if connected {
            theme::accent_cyan()
        } else {
            theme::fg_muted()
        };
        let node_id = target.node_id.clone();

        let mut chip = div()
            .id(format!("transfer-target-chip-{}", target.node_id))
            .min_w(px(146.0))
            .px(px(12.0))
            .py(px(10.0))
            .rounded(px(16.0))
            .bg(if selected {
                theme::bg_panel_highlight()
            } else {
                theme::bg_console()
            })
            .border_1()
            .border_color(if selected {
                accent.opacity(0.34)
            } else {
                theme::border_soft()
            })
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .justify_between()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .font_semibold()
                            .text_color(if connected {
                                theme::fg_primary()
                            } else {
                                theme::fg_secondary()
                            })
                            .child(target.device_id.clone()),
                    )
                    .child(
                        div()
                            .h_flex()
                            .items_center()
                            .gap(px(6.0))
                            .child(div().size(px(6.0)).rounded(px(999.0)).bg(accent))
                            .child(
                                div()
                                    .text_size(px(10.0))
                                    .font_semibold()
                                    .text_color(accent)
                                    .child(if connected { "Connected" } else { "Offline" }),
                            ),
                    ),
            );

        if connected {
            chip = chip
                .cursor_pointer()
                .hover(|this| {
                    this.bg(theme::bg_panel_alt())
                        .border_color(theme::border_strong())
                })
                .active(|this| this.bg(theme::bg_panel()))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.toggle_transfer_target(&node_id, cx);
                }));
        } else {
            chip = chip.opacity(0.72);
        }

        chip.into_any_element()
    }
}
