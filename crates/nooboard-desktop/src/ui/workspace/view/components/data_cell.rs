use gpui::{Div, ParentElement, Styled, div, px};
use gpui_component::StyledExt;

use crate::ui::theme;

pub(crate) fn data_cell(
    label: &str,
    value: impl Into<String>,
    note: impl Into<String>,
    accent: gpui::Hsla,
) -> Div {
    let value = value.into();
    let note = note.into();

    div()
        .v_flex()
        .gap(px(10.0))
        .p(px(14.0))
        .bg(theme::bg_panel_alt())
        .border_1()
        .border_color(theme::border_base())
        .rounded(px(18.0))
        .child(div().h(px(2.0)).w_full().bg(accent).rounded(px(999.0)))
        .child(
            div()
                .text_size(px(11.0))
                .font_semibold()
                .text_color(theme::fg_secondary())
                .child(label.to_uppercase()),
        )
        .child(
            div()
                .text_size(px(16.0))
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
                .child(note),
        )
}
