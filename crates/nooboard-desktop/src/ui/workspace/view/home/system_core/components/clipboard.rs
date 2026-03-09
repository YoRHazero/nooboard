use gpui::{AnyElement, Div, Hsla, ParentElement, Styled, div, px};
use gpui_component::{Icon, IconName, StyledExt};

use crate::ui::theme;

pub(in crate::ui::workspace::view::home::system_core) fn clipboard_action_shell(
    accent: Hsla,
) -> Div {
    div()
        .size(px(34.0))
        .cursor_pointer()
        .rounded(px(12.0))
        .bg(theme::bg_panel_alt())
        .border_1()
        .border_color(accent.opacity(0.22))
        .flex()
        .items_center()
        .justify_center()
}

pub(in crate::ui::workspace::view::home::system_core) fn clipboard_action_placeholder(
    accent: Hsla,
) -> Div {
    div()
        .size(px(34.0))
        .rounded(px(12.0))
        .bg(theme::bg_panel_alt())
        .border_1()
        .border_color(theme::border_soft())
        .flex()
        .items_center()
        .justify_center()
        .opacity(0.56)
        .child(
            Icon::new(IconName::Copy)
                .size(px(15.0))
                .text_color(accent.opacity(0.9)),
        )
}

pub(in crate::ui::workspace::view::home::system_core) fn clipboard_read_board(
    device_id: String,
    recorded_at_label: String,
    accent: Hsla,
    action: AnyElement,
    content: String,
) -> Div {
    div()
        .relative()
        .v_flex()
        .size_full()
        .overflow_hidden()
        .child(
            div()
                .v_flex()
                .size_full()
                .gap(px(16.0))
                .p(px(18.0))
                .child(
                    div()
                        .h_flex()
                        .justify_between()
                        .items_start()
                        .gap(px(12.0))
                        .child(
                            div()
                                .v_flex()
                                .gap(px(6.0))
                                .child(
                                    div()
                                        .h_flex()
                                        .items_center()
                                        .gap(px(10.0))
                                        .child(div().size(px(8.0)).rounded(px(999.0)).bg(accent))
                                        .child(
                                            div()
                                                .text_size(px(13.0))
                                                .font_semibold()
                                                .text_color(theme::fg_primary())
                                                .truncate()
                                                .child(device_id),
                                        ),
                                )
                                .child(
                                    div()
                                        .text_size(px(10.0))
                                        .font_semibold()
                                        .text_color(theme::fg_muted())
                                        .child(recorded_at_label),
                                ),
                        )
                        .child(action),
                )
                .child(
                    div()
                        .w_full()
                        .h(px(1.0))
                        .bg(theme::border_soft().opacity(0.94)),
                )
                .child(
                    div().relative().flex_1().min_h(px(0.0)).child(
                        div()
                            .absolute()
                            .top(px(0.0))
                            .left(px(0.0))
                            .right(px(0.0))
                            .bottom(px(0.0))
                            .text_size(px(14.0))
                            .text_color(theme::fg_primary())
                            .line_clamp(12)
                            .text_ellipsis()
                            .child(content),
                    ),
                ),
        )
}
