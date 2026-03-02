use gpui::{AnimationExt as _, IntoElement, Styled, div, px};

use super::super::shared::pulse_animation;

pub(crate) fn pulse_beacon(id: &'static str, accent: gpui::Hsla) -> impl IntoElement {
    div()
        .size(px(10.0))
        .rounded(px(999.0))
        .bg(accent)
        .shadow_xs()
        .with_animation(id, pulse_animation(), move |this, delta| {
            this.opacity(0.55 + delta * 0.45)
                .bg(accent.opacity(0.65 + delta * 0.35))
        })
}
