use gpui::{
    Context, Div, Hsla, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement,
    Styled, div, px,
};
use gpui_component::{Icon, IconName, StyledExt};

use crate::ui::theme;

use super::snapshot::{PeerDuplicateWarning, PeersSnapshot};
use super::{
    WorkspaceView, page_state::PeersFilter, peers_filter_chip as peers_filter_chip_control,
    peers_panel_shell, peers_summary_card,
};

impl WorkspaceView {
    pub(super) fn peers_header(&self, snapshot: &PeersSnapshot, cx: &mut Context<Self>) -> Div {
        let counts = snapshot.counts;
        let mut content = div().w_full().v_flex().gap(px(14.0)).child(
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
                                .child("Live connected peers mirrored from the app service."),
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
                            counts.all,
                            theme::accent_cyan(),
                            cx,
                        ))
                        .child(self.peers_filter_chip(
                            "peers-filter-idle",
                            PeersFilter::Idle,
                            counts.idle,
                            theme::accent_green(),
                            cx,
                        ))
                        .child(self.peers_filter_chip(
                            "peers-filter-transferring",
                            PeersFilter::Transferring,
                            counts.transferring,
                            theme::accent_blue(),
                            cx,
                        )),
                ),
        );

        if let Some(warning) = snapshot.duplicate_warning.as_ref() {
            content = content.child(self.peers_duplicate_warning(warning));
        }

        content.child(
            div()
                .h_flex()
                .items_stretch()
                .gap(px(12.0))
                .child(peers_summary_card(
                    "Connected",
                    counts.all,
                    "currently connected peers",
                    IconName::Network,
                    theme::accent_cyan(),
                ))
                .child(peers_summary_card(
                    "Idle",
                    counts.idle,
                    "connected with no active transfer",
                    IconName::CircleCheck,
                    theme::accent_green(),
                ))
                .child(peers_summary_card(
                    "Transferring",
                    counts.transferring,
                    "connected with active file lanes",
                    IconName::Replace,
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

    fn peers_duplicate_warning(&self, warning: &PeerDuplicateWarning) -> Div {
        div()
            .w_full()
            .h_flex()
            .items_center()
            .gap(px(12.0))
            .p(px(14.0))
            .bg(theme::accent_amber().opacity(0.1))
            .border_1()
            .border_color(theme::accent_amber().opacity(0.28))
            .rounded(px(18.0))
            .child(
                div()
                    .size(px(30.0))
                    .rounded(px(10.0))
                    .bg(theme::accent_amber().opacity(0.16))
                    .border_1()
                    .border_color(theme::accent_amber().opacity(0.28))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        Icon::new(IconName::TriangleAlert)
                            .size(px(14.0))
                            .text_color(theme::accent_amber()),
                    ),
            )
            .child(
                div()
                    .v_flex()
                    .gap(px(4.0))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .font_semibold()
                            .text_color(theme::accent_amber())
                            .child(warning.title()),
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(theme::fg_secondary())
                            .child(warning.detail()),
                    ),
            )
    }
}
