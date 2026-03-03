mod panels;

use gpui::{
    AnimationExt as _, Context, Div, Hsla, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, div, px,
};
use gpui_component::StyledExt;
use gpui_component::scroll::ScrollableElement;

use crate::ui::theme;

use super::{
    WorkspaceView,
    components::console_pill,
    shared::{ACTIVITY_COLLAPSED_WIDTH, ACTIVITY_WIDTH, panel_toggle_animation},
};

const ACTIVITY_HANDLE_WIDTH: f32 = 28.0;
const ACTIVITY_HANDLE_HEIGHT: f32 = 88.0;
const ACTIVITY_HANDLE_OFFSET: f32 = 14.0;

impl WorkspaceView {
    fn activity_rail_handle(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let expanded = self.activity_rail_expanded;
        let triangle = if expanded { "▶" } else { "◀" };
        let accent = if expanded {
            theme::accent_cyan()
        } else {
            theme::accent_green()
        };

        div()
            .id("activity-rail-handle")
            .cursor_pointer()
            .on_click(cx.listener(|this, _, _, cx| {
                this.activity_rail_expanded = !this.activity_rail_expanded;
                this.activity_rail_has_toggled = true;
                cx.notify();
            }))
            .w(px(ACTIVITY_HANDLE_WIDTH))
            .h(px(ACTIVITY_HANDLE_HEIGHT))
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

    fn activity_rail_handle_slot(&self, cx: &mut Context<Self>) -> Div {
        div()
            .absolute()
            .left(px(-ACTIVITY_HANDLE_OFFSET))
            .top(px(0.0))
            .bottom(px(0.0))
            .w(px(ACTIVITY_HANDLE_WIDTH))
            .flex()
            .items_center()
            .justify_center()
            .child(self.activity_rail_handle(cx))
    }

    fn activity_rail_trace(&self, accent: Hsla, left: f32) -> Div {
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

    fn collapsed_activity_rail(&self, cx: &mut Context<Self>) -> Div {
        div()
            .relative()
            .size_full()
            .child(self.activity_rail_trace(theme::accent_green(), 0.0))
            .child(self.activity_rail_handle_slot(cx))
    }

    fn expanded_activity_rail(&self) -> Div {
        div()
            .v_flex()
            .h_full()
            .gap(px(18.0))
            .px(px(16.0))
            .py(px(16.0))
            .child(
                div()
                    .v_flex()
                    .gap(px(12.0))
                    .p(px(14.0))
                    .bg(theme::bg_rail_panel())
                    .border_1()
                    .border_color(theme::border_soft())
                    .rounded(px(22.0))
                    .child(
                        div()
                            .h_flex()
                            .items_center()
                            .gap(px(10.0))
                            .child(
                                div()
                                    .h_flex()
                                    .items_center()
                                    .gap(px(10.0))
                                    .child(div().size(px(10.0)).rounded(px(999.0)).bg(theme::accent_cyan()))
                                    .child(
                                        div()
                                            .text_size(px(16.0))
                                            .font_semibold()
                                            .text_color(theme::fg_primary())
                                            .child("Operations Feed"),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .h_flex()
                            .gap(px(8.0))
                            .items_center()
                            .child(console_pill("telemetry", theme::accent_cyan()))
                            .child(console_pill("review", theme::accent_amber()))
                            .child(console_pill("notes", theme::accent_rose())),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(theme::fg_secondary())
                            .line_clamp(2)
                            .text_ellipsis()
                            .child("A side telemetry column with tighter framing and darker panel treatment, intentionally distinct from the main canvas surfaces."),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scrollbar()
                    .child(
                        div()
                            .v_flex()
                            .gap(px(20.0))
                            .child(self.activity_panel())
                            .child(self.pending_files_panel())
                            .child(self.error_panel()),
                    ),
            )
    }

    pub(super) fn activity_rail(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let expanded = self.activity_rail_expanded;
        let width = if expanded {
            ACTIVITY_WIDTH
        } else {
            ACTIVITY_COLLAPSED_WIDTH
        };
        let from_width = if expanded {
            ACTIVITY_COLLAPSED_WIDTH
        } else {
            ACTIVITY_WIDTH
        };
        let animation_id = if expanded {
            "activity-rail-width-expand"
        } else {
            "activity-rail-width-collapse"
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
                        .child(self.activity_rail_trace(theme::accent_cyan(), 0.0))
                        .child(self.expanded_activity_rail()),
                )
                .child(self.activity_rail_handle_slot(cx))
        } else {
            div()
                .relative()
                .w(px(width))
                .h_full()
                .min_h_0()
                .child(self.collapsed_activity_rail(cx))
        };

        if self.activity_rail_has_toggled {
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
}
