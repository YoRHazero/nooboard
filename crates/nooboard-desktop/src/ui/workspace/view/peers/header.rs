use gpui::{
    Context, Div, Hsla, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, div, px,
};
use gpui_component::{Icon, IconName, StyledExt};

use crate::ui::theme;

use super::{WorkspaceView, page_state::PeersFilter};

impl WorkspaceView {
    pub(super) fn peers_header(&self, cx: &mut Context<Self>) -> Div {
        let (total, connected, transferring) = self.peer_counts();

        div()
            .w_full()
            .v_flex()
            .gap(px(14.0))
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .justify_between()
                    .gap(px(16.0))
                    .p(px(18.0))
                    .bg(theme::bg_panel())
                    .border_1()
                    .border_color(theme::border_base())
                    .rounded(px(24.0))
                    .shadow_xs()
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
                    .child(self.peers_summary_card(
                        "Total",
                        total,
                        "all discovered peers",
                        IconName::Network,
                        theme::accent_cyan(),
                    ))
                    .child(self.peers_summary_card(
                        "Connected",
                        connected,
                        "stable links",
                        IconName::CircleCheck,
                        theme::accent_green(),
                    ))
                    .child(self.peers_summary_card(
                        "Transferring",
                        transferring,
                        "active file lanes",
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

        div()
            .id(id)
            .h_flex()
            .items_center()
            .gap(px(8.0))
            .px(px(12.0))
            .py(px(8.0))
            .cursor_pointer()
            .bg(if active {
                theme::bg_panel_highlight()
            } else {
                theme::bg_console()
            })
            .border_1()
            .border_color(if active {
                accent.opacity(0.34)
            } else {
                theme::border_soft()
            })
            .rounded(px(14.0))
            .hover(|this| {
                this.bg(theme::bg_panel_alt())
                    .border_color(theme::border_strong())
            })
            .active(|this| this.bg(theme::bg_panel()))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.set_peers_filter(filter, cx);
            }))
            .child(
                div()
                    .text_size(px(11.0))
                    .font_semibold()
                    .text_color(if active { accent } else { theme::fg_secondary() })
                    .child(filter.label()),
            )
            .child(
                div()
                    .px(px(6.0))
                    .py(px(2.0))
                    .rounded(px(999.0))
                    .bg(accent.opacity(if active { 0.2 } else { 0.12 }))
                    .border_1()
                    .border_color(accent.opacity(if active { 0.38 } else { 0.24 }))
                    .text_size(px(10.0))
                    .font_semibold()
                    .text_color(accent)
                    .child(count.to_string()),
            )
    }

    fn peers_summary_card(
        &self,
        label: &'static str,
        value: usize,
        hint: &'static str,
        icon: IconName,
        accent: Hsla,
    ) -> Div {
        div()
            .flex_1()
            .min_w(px(0.0))
            .v_flex()
            .gap(px(10.0))
            .p(px(14.0))
            .bg(theme::bg_panel())
            .border_1()
            .border_color(theme::border_base())
            .rounded(px(20.0))
            .shadow_xs()
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .justify_between()
                    .gap(px(10.0))
                    .child(
                        div()
                            .h_flex()
                            .items_center()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .size(px(30.0))
                                    .rounded(px(10.0))
                                    .bg(accent.opacity(0.14))
                                    .border_1()
                                    .border_color(accent.opacity(0.28))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(Icon::new(icon).size(px(14.0)).text_color(accent)),
                            )
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .font_semibold()
                                    .text_color(theme::fg_secondary())
                                    .child(label.to_string()),
                            ),
                    )
                    .child(div().size(px(6.0)).rounded(px(999.0)).bg(accent)),
            )
            .child(
                div()
                    .text_size(px(30.0))
                    .font_semibold()
                    .text_color(theme::fg_primary())
                    .child(value.to_string()),
            )
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(theme::fg_muted())
                    .line_clamp(1)
                    .text_ellipsis()
                    .child(hint.to_string()),
            )
    }
}
