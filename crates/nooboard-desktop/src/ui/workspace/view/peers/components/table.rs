use gpui::{Div, InteractiveElement, ParentElement, Styled, div, px};
use gpui_component::StyledExt;

use crate::ui::theme;

const DEVICE_COL_WIDTH: f32 = 160.0;
const IP_COL_WIDTH: f32 = 208.0;
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
        .child(peers_header_cell("device_id", Some(DEVICE_COL_WIDTH)))
        .child(
            peers_header_cell("noob_id", None)
                .flex_1()
                .min_w(px(NOOB_ID_MIN_WIDTH)),
        )
        .child(peers_header_cell("ip", Some(IP_COL_WIDTH)))
        .child(peers_header_cell("status", Some(STATUS_COL_WIDTH)))
}

pub(in crate::ui::workspace::view::peers) fn peers_table_row(
    index: usize,
    device_id: String,
    noob_id: String,
    ip: String,
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
            theme::bg_console()
        } else {
            theme::bg_panel_alt()
        })
        .border_1()
        .border_color(theme::border_soft())
        .rounded(px(14.0))
        .child(
            div()
                .w(px(DEVICE_COL_WIDTH))
                .flex_shrink_0()
                .text_size(px(12.0))
                .font_semibold()
                .text_color(theme::fg_primary())
                .truncate()
                .child(device_id),
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
                .w(px(IP_COL_WIDTH))
                .flex_shrink_0()
                .text_size(px(12.0))
                .text_color(theme::fg_secondary())
                .truncate()
                .child(ip),
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
