use gpui::{AnyElement, Div, Hsla, ParentElement, Styled, div, px};
use gpui_component::{Icon, IconName, StyledExt};

use crate::ui::theme;

pub(in crate::ui::workspace::view::home) fn recent_activity_card_shell() -> Div {
    div()
        .v_flex()
        .gap(px(18.0))
        .p(px(22.0))
        .bg(theme::bg_panel())
        .border_1()
        .border_color(theme::border_base())
        .rounded(px(24.0))
        .shadow_xs()
}

pub(in crate::ui::workspace::view::home) fn recent_activity_card_header(item_count: usize) -> Div {
    div()
        .h_flex()
        .items_end()
        .justify_between()
        .gap(px(16.0))
        .child(
            div()
                .v_flex()
                .gap(px(8.0))
                .child(
                    div()
                        .text_size(px(11.0))
                        .font_semibold()
                        .text_color(theme::accent_cyan())
                        .child("RECENT ACTIVITY"),
                )
                .child(
                    div()
                        .text_size(px(24.0))
                        .font_semibold()
                        .text_color(theme::fg_primary())
                        .child("Recent Activity"),
                ),
        )
        .child(
            div()
                .text_size(px(12.0))
                .text_color(theme::fg_muted())
                .child(format!("{} items", item_count)),
        )
}

pub(in crate::ui::workspace::view::home) fn recent_activity_row(
    kind_label: String,
    time_label: String,
    title: String,
    icon: IconName,
    accent: Hsla,
    copy_action: Option<AnyElement>,
) -> Div {
    let label_row = {
        let row = div().h_flex().items_center().gap(px(8.0)).child(
            div()
                .px(px(10.0))
                .py(px(5.0))
                .rounded(px(999.0))
                .bg(accent.opacity(0.14))
                .border_1()
                .border_color(accent.opacity(0.28))
                .text_size(px(10.0))
                .font_semibold()
                .text_color(accent)
                .child(kind_label),
        );

        match copy_action {
            Some(action) => row.child(action),
            None => row,
        }
    };

    div()
        .h_flex()
        .items_start()
        .gap(px(14.0))
        .p(px(16.0))
        .bg(theme::bg_panel_alt())
        .border_1()
        .border_color(theme::border_soft())
        .rounded(px(20.0))
        .child(
            div()
                .mt(px(2.0))
                .size(px(34.0))
                .rounded(px(12.0))
                .bg(accent.opacity(0.14))
                .border_1()
                .border_color(accent.opacity(0.28))
                .flex()
                .items_center()
                .justify_center()
                .child(Icon::new(icon).size(px(16.0)).text_color(accent)),
        )
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .v_flex()
                .gap(px(8.0))
                .child(
                    div()
                        .h_flex()
                        .items_center()
                        .justify_between()
                        .gap(px(12.0))
                        .child(label_row)
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(theme::fg_muted())
                                .child(time_label),
                        ),
                )
                .child(
                    div()
                        .text_size(px(14.0))
                        .font_semibold()
                        .text_color(theme::fg_primary())
                        .line_clamp(2)
                        .text_ellipsis()
                        .child(title),
                ),
        )
}
