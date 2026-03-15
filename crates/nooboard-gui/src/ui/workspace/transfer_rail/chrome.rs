use gpui::{
    ClickEvent, Context, Div, Hsla, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use gpui_component::StyledExt;
use gpui_component::scroll::ScrollableElement;

use crate::{ui::theme, workspace::view_state::TransfersPageViewState};

use super::super::WorkspaceView;

const TRANSFER_RAIL_HANDLE_WIDTH: f32 = 28.0;
const TRANSFER_RAIL_HANDLE_HEIGHT: f32 = 88.0;
const TRANSFER_RAIL_HANDLE_OFFSET: f32 = 14.0;

impl WorkspaceView {
    pub(in crate::ui::workspace::transfer_rail) fn transfer_rail_handle(
        &self,
        on_toggle: impl Fn(&mut Self, &ClickEvent, &mut Window, &mut Context<Self>) + 'static,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let expanded = self.transfer_rail_expanded;
        let triangle = if expanded { "▶" } else { "◀" };
        let accent = if expanded {
            theme::accent_blue()
        } else {
            theme::accent_green()
        };

        div()
            .id("transfer-rail-handle")
            .cursor_pointer()
            .on_click(cx.listener(on_toggle))
            .w(px(TRANSFER_RAIL_HANDLE_WIDTH))
            .h(px(TRANSFER_RAIL_HANDLE_HEIGHT))
            .rounded(px(16.0))
            .bg(theme::bg_console())
            .border_1()
            .border_color(accent.opacity(0.28))
            .shadow_xs()
            .hover(|this| {
                this.bg(theme::bg_panel_highlight())
                    .border_color(accent.opacity(0.42))
            })
            .active(|this| this.bg(theme::bg_panel_alt()))
            .child(
                div()
                    .v_flex()
                    .size_full()
                    .items_center()
                    .justify_between()
                    .py(px(10.0))
                    .child(
                        div()
                            .w(px(2.0))
                            .h(px(14.0))
                            .rounded(px(999.0))
                            .bg(accent.opacity(0.65)),
                    )
                    .child(
                        div()
                            .text_size(px(14.0))
                            .font_semibold()
                            .text_color(accent)
                            .child(triangle),
                    )
                    .child(
                        div()
                            .w(px(2.0))
                            .h(px(14.0))
                            .rounded(px(999.0))
                            .bg(accent.opacity(0.65)),
                    ),
            )
    }

    pub(in crate::ui::workspace::transfer_rail) fn transfer_rail_handle_slot(
        &self,
        on_toggle: impl Fn(&mut Self, &ClickEvent, &mut Window, &mut Context<Self>) + 'static,
        cx: &Context<Self>,
    ) -> Div {
        div()
            .absolute()
            .left(px(-TRANSFER_RAIL_HANDLE_OFFSET))
            .top(px(0.0))
            .bottom(px(0.0))
            .w(px(TRANSFER_RAIL_HANDLE_WIDTH))
            .flex()
            .items_center()
            .justify_center()
            .child(self.transfer_rail_handle(on_toggle, cx))
    }

    pub(in crate::ui::workspace::transfer_rail) fn transfer_rail_trace(
        &self,
        accent: Hsla,
        left: f32,
    ) -> Div {
        div()
            .absolute()
            .left(px(left))
            .top(px(0.0))
            .bottom(px(0.0))
            .w(px(2.0))
            .child(
                div()
                    .absolute()
                    .left(px(0.0))
                    .top(px(28.0))
                    .bottom(px(28.0))
                    .w(px(1.0))
                    .bg(theme::border_soft().opacity(0.88)),
            )
            .child(
                div()
                    .absolute()
                    .left(px(0.0))
                    .top(px(108.0))
                    .h(px(54.0))
                    .w(px(2.0))
                    .rounded(px(999.0))
                    .bg(accent.opacity(0.32)),
            )
    }

    pub(in crate::ui::workspace::transfer_rail) fn collapsed_transfer_rail(
        &self,
        on_toggle: impl Fn(&mut Self, &ClickEvent, &mut Window, &mut Context<Self>) + 'static,
        cx: &Context<Self>,
    ) -> Div {
        div()
            .relative()
            .size_full()
            .child(self.transfer_rail_trace(theme::accent_green(), 0.0))
            .child(self.transfer_rail_handle_slot(on_toggle, cx))
    }

    pub(in crate::ui::workspace::transfer_rail) fn expanded_transfer_rail(
        &self,
        page: Option<&TransfersPageViewState>,
        on_open: impl Fn(&mut Self, &ClickEvent, &mut Window, &mut Context<Self>) + Clone + 'static,
        cx: &mut Context<Self>,
    ) -> Div {
        match page {
            Some(page) => div()
                .v_flex()
                .h_full()
                .gap(px(18.0))
                .px(px(16.0))
                .py(px(16.0))
                .child(
                    div()
                        .v_flex()
                        .gap(px(16.0))
                        .p(px(16.0))
                        .bg(theme::bg_rail_panel())
                        .border_1()
                        .border_color(theme::border_soft())
                        .rounded(px(22.0))
                        .child(self.transfer_summary(page, on_open.clone(), cx)),
                )
                .child(
                    div().flex_1().min_h_0().child(
                        div()
                            .size_full()
                            .overflow_y_scrollbar()
                            .child(self.transfer_sections(page, on_open, cx)),
                    ),
                ),
            None => div()
                .v_flex()
                .h_full()
                .gap(px(18.0))
                .px(px(16.0))
                .py(px(16.0))
                .child(
                    div()
                        .v_flex()
                        .gap(px(10.0))
                        .p(px(16.0))
                        .bg(theme::bg_rail_panel())
                        .border_1()
                        .border_color(theme::border_soft())
                        .rounded(px(22.0))
                        .child(
                            div()
                                .text_size(px(12.0))
                                .font_semibold()
                                .text_color(theme::accent_cyan())
                                .child("TRANSFER STATUS"),
                        )
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(theme::fg_muted())
                                .child("Waiting for workspace snapshot."),
                        ),
                ),
        }
    }
}
