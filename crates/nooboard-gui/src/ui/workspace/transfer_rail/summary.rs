use gpui::{
    ClickEvent, Context, Hsla, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use gpui_component::StyledExt;

use crate::{ui::theme, workspace::view_state::TransfersPageViewState};

use super::super::WorkspaceView;

impl WorkspaceView {
    pub(in crate::ui::workspace::transfer_rail) fn transfer_summary(
        &self,
        state: &TransfersPageViewState,
        on_open: impl Fn(&mut Self, &ClickEvent, &mut Window, &mut Context<Self>) + Clone + 'static,
        cx: &Context<Self>,
    ) -> gpui::Div {
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
                        "Awaiting",
                        state.incoming.len(),
                        theme::accent_amber(),
                        on_open.clone(),
                        cx,
                    ))
                    .child(self.transfer_summary_card(
                        "Active",
                        state.active.len(),
                        theme::accent_blue(),
                        on_open.clone(),
                        cx,
                    ))
                    .child(self.transfer_summary_card(
                        "Completed",
                        state.completed.len(),
                        theme::accent_green(),
                        on_open,
                        cx,
                    )),
            )
    }

    fn transfer_summary_card(
        &self,
        label: &str,
        count: usize,
        accent: Hsla,
        on_open: impl Fn(&mut Self, &ClickEvent, &mut Window, &mut Context<Self>) + 'static,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        div()
            .id(format!("transfer-summary-{label}"))
            .cursor_pointer()
            .on_click(cx.listener(on_open))
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
