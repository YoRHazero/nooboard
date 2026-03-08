use gpui::{
    Context, Div, Hsla, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement,
    Styled, div, px,
};
use gpui_component::StyledExt;

use crate::ui::theme;

use super::{
    WorkspaceView, page_state::PeersFilter, peers_filter_chip as peers_filter_chip_control,
    peers_panel_shell, peers_summary_card,
};

impl WorkspaceView {
    pub(super) fn peers_header(&self, cx: &mut Context<Self>) -> Div {
        let (total, connected, transferring) = self.peer_counts();

        div()
            .w_full()
            .v_flex()
            .gap(px(14.0))
            .child(
                peers_panel_shell()
                    .rounded(px(24.0))
                    .h_flex()
                    .items_center()
                    .justify_between()
                    .gap(px(16.0))
                    .p(px(18.0))
                    .child(
                        div()
                            .v_flex()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .text_size(px(24.0))
                                    .font_semibold()
                                    .text_color(theme::fg_primary())
                                    .child("Peers & Network"),
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(theme::fg_muted())
                                    .child("Connected tab view based on stage5-wireframe."),
                            ),
                    )
                    .child(
                        div()
                            .h_flex()
                            .items_center()
                            .gap(px(8.0))
                            .child(self.peers_filter_chip(
                                "peers-filter-all",
                                PeersFilter::All,
                                total,
                                theme::accent_cyan(),
                                cx,
                            ))
                            .child(self.peers_filter_chip(
                                "peers-filter-connected",
                                PeersFilter::Connected,
                                connected,
                                theme::accent_green(),
                                cx,
                            ))
                            .child(self.peers_filter_chip(
                                "peers-filter-transferring",
                                PeersFilter::Transferring,
                                transferring,
                                theme::accent_blue(),
                                cx,
                            )),
                    ),
            )
            .child(
                div()
                    .h_flex()
                    .items_stretch()
                    .gap(px(12.0))
                    .child(peers_summary_card(
                        "Total",
                        total,
                        "all discovered peers",
                        gpui_component::IconName::Network,
                        theme::accent_cyan(),
                    ))
                    .child(peers_summary_card(
                        "Connected",
                        connected,
                        "stable links",
                        gpui_component::IconName::CircleCheck,
                        theme::accent_green(),
                    ))
                    .child(peers_summary_card(
                        "Transferring",
                        transferring,
                        "active file lanes",
                        gpui_component::IconName::Replace,
                        theme::accent_blue(),
                    )),
            )
    }

    fn peers_filter_chip(
        &self,
        id: &'static str,
        filter: PeersFilter,
        count: usize,
        accent: Hsla,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let active = self.peers_filter() == filter;

        peers_filter_chip_control(filter.label(), count, active, accent)
            .id(id)
            .cursor_pointer()
            .hover(|this| {
                this.bg(theme::bg_panel_alt())
                    .border_color(theme::border_strong())
            })
            .active(|this| this.bg(theme::bg_panel()))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.set_peers_filter(filter, cx);
            }))
    }
}
