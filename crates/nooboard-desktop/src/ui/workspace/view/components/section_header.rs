use gpui::{Div, ParentElement, Styled, div, px};
use gpui_component::StyledExt;

use crate::ui::theme;

pub(crate) fn section_header(
    eyebrow: &str,
    title: &str,
    detail: impl Into<String>,
    accent: gpui::Hsla,
) -> Div {
    let detail = detail.into();

    div()
        .h_flex()
        .items_start()
        .justify_between()
        .gap(px(16.0))
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .v_flex()
                .gap(px(8.0))
                .child(
                    div()
                        .h_flex()
                        .gap(px(8.0))
                        .items_center()
                        .child(div().size(px(8.0)).rounded(px(999.0)).bg(accent))
                        .child(
                            div()
                                .text_size(px(11.0))
                                .font_semibold()
                                .text_color(accent)
                                .child(eyebrow.to_uppercase()),
                        ),
                )
                .child(
                    div()
                        .text_size(px(24.0))
                        .font_semibold()
                        .text_color(theme::fg_primary())
                        .child(title.to_string()),
                )
                .child(
                    div()
                        .text_size(px(13.0))
                        .text_color(theme::fg_secondary())
                        .line_clamp(2)
                        .text_ellipsis()
                        .child(detail),
                ),
        )
}
