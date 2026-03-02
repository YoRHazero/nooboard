use gpui::{AnimationExt as _, IntoElement, ParentElement, Styled, div, px};
use gpui_component::{Icon, IconName, StyledExt};

use crate::ui::theme;

use super::super::shared::enter_animation;

pub(crate) fn summary_card(
    id: &'static str,
    label: &str,
    value: String,
    hint: impl Into<String>,
    icon: IconName,
    accent: gpui::Hsla,
) -> impl IntoElement {
    let hint = hint.into();

    div()
        .flex_1()
        .min_w(px(0.0))
        .v_flex()
        .gap(px(12.0))
        .p(px(18.0))
        .bg(theme::bg_panel())
        .border_1()
        .border_color(theme::border_base())
        .rounded(px(22.0))
        .shadow_xs()
        .child(div().h(px(3.0)).w_full().bg(accent).rounded(px(999.0)))
        .child(
            div()
                .h_flex()
                .justify_between()
                .items_center()
                .gap(px(12.0))
                .child(
                    div()
                        .h_flex()
                        .gap(px(10.0))
                        .items_center()
                        .child(
                            div()
                                .size(px(34.0))
                                .rounded(px(12.0))
                                .bg(accent.opacity(0.14))
                                .border_1()
                                .border_color(accent.opacity(0.3))
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(Icon::new(icon).size(px(16.0)).text_color(accent)),
                        )
                        .child(
                            div()
                                .text_size(px(11.0))
                                .font_semibold()
                                .text_color(theme::fg_secondary())
                                .child(label.to_uppercase()),
                        ),
                )
                .child(
                    div()
                        .size(px(7.0))
                        .rounded(px(999.0))
                        .bg(accent.opacity(0.8)),
                ),
        )
        .child(
            div()
                .text_size(px(34.0))
                .font_semibold()
                .text_color(theme::fg_primary())
                .child(value),
        )
        .child(
            div()
                .text_size(px(12.0))
                .text_color(theme::fg_muted())
                .line_clamp(2)
                .text_ellipsis()
                .child(hint),
        )
        .with_animation(id, enter_animation(), |this, delta| {
            this.opacity(0.35 + delta * 0.65)
        })
}
