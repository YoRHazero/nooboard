use gpui::{Div, InteractiveElement, IntoElement, ParentElement, Styled, div, px};
use gpui_component::{Icon, IconName, StyledExt};

use crate::{
    state::{SystemPeer, SystemPeerStatus},
    ui::theme,
};

use super::WorkspaceView;

const DEVICE_COL_WIDTH: f32 = 160.0;
const IP_COL_WIDTH: f32 = 208.0;
const STATUS_COL_WIDTH: f32 = 154.0;

impl WorkspaceView {
    pub(super) fn peers_list_panel(&self) -> Div {
        let peers = self.filtered_peers();
        let filter = self.peers_filter();
        let count = peers.len();

        div()
            .w_full()
            .v_flex()
            .gap(px(12.0))
            .p(px(18.0))
            .bg(theme::bg_panel())
            .border_1()
            .border_color(theme::border_base())
            .rounded(px(24.0))
            .shadow_xs()
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .justify_between()
                    .gap(px(12.0))
                    .child(
                        div()
                            .text_size(px(18.0))
                            .font_semibold()
                            .text_color(theme::fg_primary())
                            .child("Connected tab"),
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_semibold()
                            .text_color(theme::fg_muted())
                            .child(format!("{} · {} peers", filter.label(), count)),
                    ),
            )
            .child(div().h(px(1.0)).w_full().bg(theme::border_soft()))
            .child(if peers.is_empty() {
                self.peers_empty_state().into_any_element()
            } else {
                div()
                    .v_flex()
                    .gap(px(8.0))
                    .child(self.peers_table_header())
                    .children(
                        peers
                            .into_iter()
                            .enumerate()
                            .map(|(index, peer)| self.peers_table_row(index, peer)),
                    )
                    .into_any_element()
            })
    }

    fn peers_table_header(&self) -> Div {
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
            .child(self.peers_header_cell("device_id", DEVICE_COL_WIDTH))
            .child(
                self.peers_header_cell("noob_id", 0.0)
                    .flex_1()
                    .min_w(px(220.0)),
            )
            .child(self.peers_header_cell("ip", IP_COL_WIDTH))
            .child(self.peers_header_cell("status", STATUS_COL_WIDTH))
    }

    fn peers_table_row(&self, index: usize, peer: &SystemPeer) -> impl IntoElement {
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
                    .child(peer.device_id.clone()),
            )
            .child(
                div()
                    .flex_1()
                    .min_w(px(220.0))
                    .text_size(px(12.0))
                    .text_color(theme::fg_secondary())
                    .truncate()
                    .child(peer.noob_id.clone()),
            )
            .child(
                div()
                    .w(px(IP_COL_WIDTH))
                    .flex_shrink_0()
                    .text_size(px(12.0))
                    .text_color(theme::fg_secondary())
                    .truncate()
                    .child(peer.ip.clone()),
            )
            .child(
                div()
                    .w(px(STATUS_COL_WIDTH))
                    .flex_shrink_0()
                    .child(self.peer_status_badge(peer.status)),
            )
    }

    fn peers_header_cell(&self, label: &'static str, width: f32) -> Div {
        let base = div()
            .text_size(px(10.0))
            .font_semibold()
            .text_color(theme::fg_muted())
            .child(label.to_uppercase());

        if width == 0.0 {
            base
        } else {
            base.w(px(width)).flex_shrink_0()
        }
    }

    fn peer_status_badge(&self, status: SystemPeerStatus) -> Div {
        let accent = Self::peer_status_accent(status);

        div()
            .h_flex()
            .items_center()
            .gap(px(8.0))
            .px(px(10.0))
            .py(px(5.0))
            .bg(accent.opacity(0.13))
            .border_1()
            .border_color(accent.opacity(0.28))
            .rounded(px(999.0))
            .child(div().size(px(6.0)).rounded(px(999.0)).bg(accent))
            .child(
                div()
                    .text_size(px(10.0))
                    .font_semibold()
                    .text_color(accent)
                    .child(Self::peer_status_label(status)),
            )
    }

    fn peers_empty_state(&self) -> Div {
        div()
            .w_full()
            .h(px(220.0))
            .v_flex()
            .items_center()
            .justify_center()
            .gap(px(12.0))
            .bg(theme::bg_console())
            .border_1()
            .border_color(theme::border_soft())
            .rounded(px(18.0))
            .child(
                div()
                    .size(px(34.0))
                    .rounded(px(12.0))
                    .bg(theme::bg_panel_alt())
                    .border_1()
                    .border_color(theme::border_base())
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        Icon::new(IconName::Globe)
                            .size(px(16.0))
                            .text_color(theme::fg_muted()),
                    ),
            )
            .child(
                div()
                    .text_size(px(14.0))
                    .font_semibold()
                    .text_color(theme::fg_primary())
                    .child("No peers match current filter"),
            )
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(theme::fg_muted())
                    .child(format!("Filter: {}", self.peers_filter().label())),
            )
    }
}
