mod sections;
mod summary;

use gpui::{
    AnimationExt as _, Context, Div, Hsla, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, div, px,
};
use gpui_component::StyledExt;
use gpui_component::scroll::ScrollableElement;

use crate::state::WorkspaceRoute;
use crate::ui::theme;

use super::{
    WorkspaceView,
    shared::{TRANSFER_RAIL_COLLAPSED_WIDTH, TRANSFER_RAIL_WIDTH, panel_toggle_animation},
};

const TRANSFER_RAIL_HANDLE_WIDTH: f32 = 28.0;
const TRANSFER_RAIL_HANDLE_HEIGHT: f32 = 88.0;
const TRANSFER_RAIL_HANDLE_OFFSET: f32 = 14.0;

impl WorkspaceView {
    fn transfer_rail_handle(&self, cx: &mut Context<Self>) -> impl IntoElement {
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
            .on_click(cx.listener(|this, _, _, cx| {
                this.transfer_rail_expanded = !this.transfer_rail_expanded;
                this.transfer_rail_has_toggled = true;
                cx.notify();
            }))
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

    fn transfer_rail_handle_slot(&self, cx: &mut Context<Self>) -> Div {
        div()
            .absolute()
            .left(px(-TRANSFER_RAIL_HANDLE_OFFSET))
            .top(px(0.0))
            .bottom(px(0.0))
            .w(px(TRANSFER_RAIL_HANDLE_WIDTH))
            .flex()
            .items_center()
            .justify_center()
            .child(self.transfer_rail_handle(cx))
    }

    fn transfer_rail_trace(&self, accent: Hsla, left: f32) -> Div {
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

    fn collapsed_transfer_rail(&self, cx: &mut Context<Self>) -> Div {
        div()
            .relative()
            .size_full()
            .child(self.transfer_rail_trace(theme::accent_green(), 0.0))
            .child(self.transfer_rail_handle_slot(cx))
    }

    fn expanded_transfer_rail(&self, cx: &mut Context<Self>) -> Div {
        div()
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
                    .child(self.transfer_summary(cx)),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scrollbar()
                    .child(self.transfer_sections(cx)),
            )
    }

    pub(super) fn transfer_rail(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let expanded = self.transfer_rail_expanded;
        let width = if expanded {
            TRANSFER_RAIL_WIDTH
        } else {
            TRANSFER_RAIL_COLLAPSED_WIDTH
        };
        let from_width = if expanded {
            TRANSFER_RAIL_COLLAPSED_WIDTH
        } else {
            TRANSFER_RAIL_WIDTH
        };
        let animation_id = if expanded {
            "transfer-rail-width-expand"
        } else {
            "transfer-rail-width-collapse"
        };

        let rail = if expanded {
            div()
                .relative()
                .w(px(width))
                .h_full()
                .min_h_0()
                .child(
                    div()
                        .relative()
                        .w_full()
                        .h_full()
                        .min_h_0()
                        .overflow_hidden()
                        .bg(theme::bg_activity())
                        .border_1()
                        .border_color(theme::border_base())
                        .rounded(px(26.0))
                        .shadow_xs()
                        .child(self.transfer_rail_trace(theme::accent_blue(), 0.0))
                        .child(self.expanded_transfer_rail(cx)),
                )
                .child(self.transfer_rail_handle_slot(cx))
        } else {
            div()
                .relative()
                .w(px(width))
                .h_full()
                .min_h_0()
                .child(self.collapsed_transfer_rail(cx))
        };

        if self.transfer_rail_has_toggled {
            rail.with_animation(
                animation_id,
                panel_toggle_animation(),
                move |this, delta| {
                    let animated_width = from_width + (width - from_width) * delta;
                    this.w(px(animated_width))
                },
            )
            .into_any_element()
        } else {
            rail.into_any_element()
        }
    }

    fn open_transfers(&mut self, cx: &mut Context<Self>) {
        self.route = WorkspaceRoute::Transfers;
        cx.notify();
    }
}
