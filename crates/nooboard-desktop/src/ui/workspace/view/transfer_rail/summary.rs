use gpui::{
    Context, Div, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement,
    Styled, div, px,
};
use gpui_component::StyledExt;

use crate::ui::theme;

use super::super::transfers::snapshot::TransfersSnapshot;
use super::WorkspaceView;

impl WorkspaceView {
    pub(super) fn transfer_summary(
        &self,
        snapshot: &TransfersSnapshot,
        cx: &mut Context<Self>,
    ) -> Div {
        div()
            .v_flex()
            .gap(px(14.0))
            .child(
                div()
                    .v_flex()
                    .gap(px(6.0))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .font_semibold()
                            .text_color(theme::accent_cyan())
                            .child("TRANSFER STATUS"),
                    )
                    .child(
                        div()
                            .text_size(px(20.0))
                            .font_semibold()
                            .text_color(theme::fg_primary())
                            .child("Transfer Status"),
                    ),
            )
            .child(
                div()
                    .grid()
                    .grid_cols(3)
                    .gap(px(10.0))
                    .child(self.transfer_summary_card(
                        0,
                        "Awaiting",
                        snapshot.metrics.awaiting,
                        theme::accent_amber(),
                        cx,
                    ))
                    .child(self.transfer_summary_card(
                        1,
                        "Active",
                        snapshot.metrics.active,
                        theme::accent_blue(),
                        cx,
                    ))
                    .child(self.transfer_summary_card(
                        2,
                        "Completed",
                        snapshot.metrics.completed,
                        theme::accent_green(),
                        cx,
                    )),
            )
    }

    fn transfer_summary_card(
        &self,
        id: usize,
        label: &str,
        count: usize,
        accent: gpui::Hsla,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .id(("transfer-summary", id))
            .cursor_pointer()
            .on_click(cx.listener(|this, _, _, cx| this.open_transfers(cx)))
            .v_flex()
            .gap(px(8.0))
            .p(px(12.0))
            .bg(theme::bg_rail_panel())
            .border_1()
            .border_color(accent.opacity(0.34))
            .rounded(px(18.0))
            .hover(|this| {
                this.bg(theme::bg_panel_highlight())
                    .border_color(accent.opacity(0.5))
            })
            .child(div().h(px(2.0)).w_full().bg(accent).rounded(px(999.0)))
            .child(
                div()
                    .text_size(px(22.0))
                    .font_semibold()
                    .text_color(theme::fg_primary())
                    .child(count.to_string()),
            )
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(theme::fg_secondary())
                    .line_clamp(2)
                    .text_ellipsis()
                    .child(label.to_string()),
            )
    }
}
