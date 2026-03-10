use gpui::{Div, InteractiveElement, ParentElement, Styled, div, prelude::FluentBuilder as _, px};
use gpui_component::{Icon, IconName, StyledExt};

use crate::ui::theme;

const DEVICE_COL_WIDTH: f32 = 180.0;
const ENDPOINT_COL_WIDTH: f32 = 240.0;
const STATUS_COL_WIDTH: f32 = 154.0;
const NOOB_ID_MIN_WIDTH: f32 = 220.0;

pub(in crate::ui::workspace::view::peers) fn peers_table_header() -> Div {
    div()
        .h_flex()
        .items_center()
        .gap(px(10.0))
        .px(px(14.0))
        .py(px(10.0))
        .bg(theme::bg_console())
        .border_1()
        .border_color(theme::border_soft())
        .rounded(px(14.0))
        .child(peers_header_cell("device", Some(DEVICE_COL_WIDTH)))
        .child(
            peers_header_cell("noob_id", None)
                .flex_1()
                .min_w(px(NOOB_ID_MIN_WIDTH)),
        )
        .child(peers_header_cell("endpoint", Some(ENDPOINT_COL_WIDTH)))
        .child(peers_header_cell("status", Some(STATUS_COL_WIDTH)))
}

pub(in crate::ui::workspace::view::peers) fn peers_table_row(
    index: usize,
    device_id: String,
    noob_id: String,
    endpoint_label: String,
    endpoint_detail: String,
    duplicate_device_id: bool,
    duplicates_local_identity: bool,
    status_badge: Div,
) -> impl gpui::IntoElement {
    div()
        .id(("peers-table-row", index))
        .h_flex()
        .items_center()
        .gap(px(10.0))
        .px(px(14.0))
        .py(px(11.0))
        .bg(if index % 2 == 0 {
            if duplicate_device_id {
                theme::accent_amber().opacity(0.08)
            } else {
                theme::bg_console()
            }
        } else {
            if duplicate_device_id {
                theme::accent_amber().opacity(0.12)
            } else {
                theme::bg_panel_alt()
            }
        })
        .border_1()
        .border_color(if duplicate_device_id {
            theme::accent_amber().opacity(0.24)
        } else {
            theme::border_soft()
        })
        .rounded(px(14.0))
        .child(
            div()
                .w(px(DEVICE_COL_WIDTH))
                .flex_shrink_0()
                .v_flex()
                .gap(px(4.0))
                .child(
                    div()
                        .h_flex()
                        .items_center()
                        .gap(px(6.0))
                        .child(
                            div()
                                .text_size(px(12.0))
                                .font_semibold()
                                .text_color(if duplicate_device_id {
                                    theme::accent_amber()
                                } else {
                                    theme::fg_primary()
                                })
                                .truncate()
                                .child(device_id),
                        )
                        .when(duplicate_device_id, |this| {
                            this.child(
                                Icon::new(IconName::TriangleAlert)
                                    .size(px(12.0))
                                    .text_color(theme::accent_amber()),
                            )
                        }),
                )
                .when(duplicate_device_id, |this| {
                    this.child(
                        div()
                            .text_size(px(10.0))
                            .font_semibold()
                            .text_color(theme::accent_amber())
                            .line_clamp(1)
                            .text_ellipsis()
                            .child(if duplicates_local_identity {
                                "matches local device label"
                            } else {
                                "duplicate device label"
                            }),
                    )
                }),
        )
        .child(
            div()
                .flex_1()
                .min_w(px(NOOB_ID_MIN_WIDTH))
                .text_size(px(12.0))
                .text_color(theme::fg_secondary())
                .truncate()
                .child(noob_id),
        )
        .child(
            div()
                .w(px(ENDPOINT_COL_WIDTH))
                .flex_shrink_0()
                .v_flex()
                .gap(px(4.0))
                .child(
                    div()
                        .text_size(px(12.0))
                        .text_color(theme::fg_secondary())
                        .truncate()
                        .child(endpoint_label),
                )
                .child(
                    div()
                        .text_size(px(10.0))
                        .text_color(theme::fg_muted())
                        .truncate()
                        .child(endpoint_detail),
                ),
        )
        .child(
            div()
                .w(px(STATUS_COL_WIDTH))
                .flex_shrink_0()
                .child(status_badge),
        )
}

fn peers_header_cell(label: &'static str, width: Option<f32>) -> Div {
    let base = div()
        .text_size(px(10.0))
        .font_semibold()
        .text_color(theme::fg_muted())
        .child(label.to_uppercase());

    match width {
        Some(width) => base.w(px(width)).flex_shrink_0(),
        None => base,
    }
}
