use gpui::{Div, IntoElement, ParentElement, Styled, div, px};
use gpui_component::StyledExt;

use crate::ui::theme;

pub(in crate::ui::workspace::view::settings) fn settings_path_field_row(
    label: &str,
    hint: &str,
    current_value: impl Into<String>,
    dirty: bool,
    field: impl IntoElement,
) -> Div {
    div()
        .v_flex()
        .gap(px(8.0))
        .child(
            div()
                .v_flex()
                .gap(px(4.0))
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(theme::fg_secondary())
                        .child(label.to_string()),
                )
                .child(
                    div()
                        .text_size(px(10.0))
                        .text_color(theme::fg_muted())
                        .line_clamp(2)
                        .text_ellipsis()
                        .child(hint.to_string()),
                )
                .child(
                    div()
                        .text_size(px(10.0))
                        .text_color(if dirty {
                            theme::accent_amber()
                        } else {
                            theme::fg_muted()
                        })
                        .line_clamp(1)
                        .text_ellipsis()
                        .child(if dirty {
                            format!("Current path: {}", current_value.into())
                        } else {
                            "Matches the current path".to_string()
                        }),
                ),
        )
        .child(field)
}

pub(in crate::ui::workspace::view::settings) fn settings_stepper_field_row(
    label: &str,
    hint: &str,
    value: impl Into<String>,
    current_value: impl Into<String>,
    step: u32,
    dirty: bool,
    decrement_button: impl IntoElement,
    increment_button: impl IntoElement,
) -> Div {
    let value = value.into();
    let current_value = current_value.into();
    let accent = if dirty {
        theme::accent_amber()
    } else {
        theme::accent_cyan()
    };

    div()
        .v_flex()
        .gap(px(8.0))
        .child(
            div()
                .h_flex()
                .items_start()
                .justify_between()
                .gap(px(12.0))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .v_flex()
                        .gap(px(4.0))
                        .child(
                            div()
                                .text_size(px(11.0))
                                .text_color(theme::fg_secondary())
                                .child(label.to_string()),
                        )
                        .child(
                            div()
                                .text_size(px(10.0))
                                .text_color(theme::fg_muted())
                                .line_clamp(2)
                                .text_ellipsis()
                                .child(hint.to_string()),
                        ),
                )
                .child(
                    div()
                        .w(px(146.0))
                        .h_flex()
                        .items_center()
                        .justify_end()
                        .gap(px(6.0))
                        .child(decrement_button)
                        .child(
                            div()
                                .min_w(px(92.0))
                                .px(px(12.0))
                                .py(px(8.0))
                                .bg(theme::bg_console())
                                .border_1()
                                .border_color(accent.opacity(0.24))
                                .rounded(px(12.0))
                                .text_size(px(12.0))
                                .font_semibold()
                                .text_color(theme::fg_primary())
                                .text_center()
                                .child(value),
                        )
                        .child(increment_button),
                ),
        )
        .child(
            div()
                .text_size(px(10.0))
                .text_color(if dirty {
                    theme::accent_amber()
                } else {
                    theme::fg_muted()
                })
                .line_clamp(1)
                .text_ellipsis()
                .child(if dirty {
                    format!("Current value: {} · step {}", current_value, step)
                } else {
                    format!("Matches the current value · step {}", step)
                }),
        )
}
