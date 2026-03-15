mod recent_activity;
mod system_core;

use std::time::Duration;

use gpui::{Animation, AnyElement, Context, IntoElement, ParentElement, Styled, div, px};
use gpui_component::StyledExt;
use gpui_component::animation::cubic_bezier;

use crate::ui::theme;

use super::{WorkspaceRenderModel, WorkspaceView};

const HOME_CONTENT_WIDTH: f32 = 735.0;

fn enter_animation() -> Animation {
    Animation::new(Duration::from_secs_f64(0.32)).with_easing(cubic_bezier(0.32, 0.72, 0.0, 1.0))
}

impl WorkspaceView {
    pub(super) fn home_page(
        &self,
        model: &WorkspaceRenderModel,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let content = div()
            .w(px(HOME_CONTENT_WIDTH))
            .max_w_full()
            .v_flex()
            .flex_shrink_0()
            .gap(px(18.0))
            .child(if let Some(state) = model.page.as_ref() {
                self.system_core_card(&state.home.system_core, cx)
                    .into_any_element()
            } else {
                self.home_loading_card().into_any_element()
            })
            .child(self.recent_activity_card(&model.shell.recent_activity, cx));

        vec![
            div()
                .w_full()
                .flex()
                .justify_center()
                .child(content)
                .into_any_element(),
        ]
    }

    fn home_loading_card(&self) -> impl IntoElement {
        div()
            .v_flex()
            .gap(px(12.0))
            .p(px(22.0))
            .bg(theme::bg_panel())
            .border_1()
            .border_color(theme::border_base())
            .rounded(px(28.0))
            .shadow_xs()
            .child(
                div()
                    .text_size(px(11.0))
                    .font_semibold()
                    .text_color(theme::accent_cyan())
                    .child("SYSTEM CORE"),
            )
            .child(
                div()
                    .text_size(px(24.0))
                    .font_semibold()
                    .text_color(theme::fg_primary())
                    .child("Waiting for workspace snapshot"),
            )
            .child(
                div()
                    .text_size(px(13.0))
                    .line_height(px(20.0))
                    .text_color(theme::fg_secondary())
                    .child(
                        "The Home surface switches to the full desktop system-core layout as soon \
                         as nooboard-core publishes the first workspace snapshot.",
                    ),
            )
    }
}
