mod chrome;
mod sections;
mod summary;

use std::time::Duration;

use gpui::{
    Animation, AnimationExt as _, ClickEvent, Context, IntoElement, ParentElement, Styled, Window,
    div, px,
};
use gpui_component::animation::cubic_bezier;

use crate::{ui::theme, workspace::route::WorkspaceRoute};

use super::{
    WorkspaceRenderModel, WorkspaceView,
    shared::{TRANSFER_RAIL_COLLAPSED_WIDTH, TRANSFER_RAIL_WIDTH},
};

fn panel_toggle_animation() -> Animation {
    Animation::new(Duration::from_secs_f64(0.24)).with_easing(cubic_bezier(0.32, 0.72, 0.0, 1.0))
}

impl WorkspaceView {
    fn open_transfers_route(&mut self, cx: &mut Context<Self>) {
        let _ = self.controller.update(cx, |controller, cx| {
            controller.set_route(WorkspaceRoute::Transfers);
            cx.notify();
        });
    }

    fn toggle_transfer_rail(&mut self, cx: &mut Context<Self>) {
        self.transfer_rail_expanded = !self.transfer_rail_expanded;
        self.transfer_rail_has_toggled = true;
        cx.notify();
    }

    pub(super) fn transfer_rail(
        &self,
        model: &WorkspaceRenderModel,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
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
        let toggle = |this: &mut Self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>| {
            this.toggle_transfer_rail(cx);
        };
        let open_transfers =
            |this: &mut Self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>| {
                this.open_transfers_route(cx);
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
                        .child(self.expanded_transfer_rail(
                            model.page.as_ref().map(|page| &page.transfers),
                            open_transfers,
                            cx,
                        )),
                )
                .child(self.transfer_rail_handle_slot(toggle, cx))
        } else {
            div()
                .relative()
                .w(px(width))
                .h_full()
                .min_h_0()
                .child(self.collapsed_transfer_rail(toggle, cx))
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
}
