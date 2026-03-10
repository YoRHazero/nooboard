use gpui::{Div, Hsla, ParentElement, Styled, div, px};
use gpui_component::StyledExt;

use crate::ui::theme;

pub(in crate::ui::workspace::view::transfers) fn transfer_target_chip(
    device_id: &str,
    noob_id: &str,
    selected: bool,
    accent: Hsla,
) -> Div {
    div()
        .min_w(px(172.0))
        .px(px(12.0))
        .py(px(10.0))
        .rounded(px(16.0))
        .bg(if selected {
            theme::bg_panel_highlight()
        } else {
            theme::bg_console()
        })
        .border_1()
        .border_color(if selected {
            accent.opacity(0.34)
        } else {
            theme::border_soft()
        })
        .child(
            div()
                .h_flex()
                .items_center()
                .justify_between()
                .gap(px(8.0))
                .child(
                    div()
                        .v_flex()
                        .gap(px(4.0))
                        .child(
                            div()
                                .text_size(px(12.0))
                                .font_semibold()
                                .text_color(theme::fg_primary())
                                .child(device_id.to_string()),
                        )
                        .child(
                            div()
                                .text_size(px(10.0))
                                .text_color(theme::fg_muted())
                                .line_clamp(1)
                                .text_ellipsis()
                                .child(noob_id.to_string()),
                        ),
                )
                .child(
                    div()
                        .h_flex()
                        .items_center()
                        .gap(px(6.0))
                        .child(div().size(px(6.0)).rounded(px(999.0)).bg(accent))
                        .child(
                            div()
                                .text_size(px(10.0))
                                .font_semibold()
                                .text_color(accent)
                                .child("Connected"),
                        ),
                ),
        )
}

pub(in crate::ui::workspace::view::transfers) fn transfer_status_badge(
    label: &str,
    accent: Hsla,
) -> Div {
    div()
        .px(px(10.0))
        .py(px(6.0))
        .rounded(px(999.0))
        .bg(accent.opacity(0.12))
        .border_1()
        .border_color(accent.opacity(0.24))
        .text_size(px(10.0))
        .font_semibold()
        .text_color(accent)
        .child(label.to_string())
}

pub(in crate::ui::workspace::view::transfers) fn transfer_card_heading(
    file_name: &str,
    accent: Hsla,
) -> Div {
    div()
        .v_flex()
        .gap(px(8.0))
        .child(div().h(px(2.0)).w_full().bg(accent).rounded(px(999.0)))
        .child(
            div()
                .text_size(px(13.0))
                .font_semibold()
                .text_color(theme::fg_primary())
                .line_clamp(2)
                .text_ellipsis()
                .child(file_name.to_string()),
        )
}

pub(in crate::ui::workspace::view::transfers) fn transfer_card_meta(
    source_device: &str,
    size_label: &str,
) -> Div {
    div()
        .text_size(px(11.0))
        .text_color(theme::fg_muted())
        .truncate()
        .child(format!("{} · {}", source_device, size_label))
}
