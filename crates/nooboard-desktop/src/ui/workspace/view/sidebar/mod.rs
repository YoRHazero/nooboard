mod navigation;

use gpui::{Context, Div, ParentElement, Styled, div, px};
use gpui_component::StyledExt;
use gpui_component::scroll::ScrollableElement;

use crate::state::WorkspaceRoute;
use crate::ui::theme;

use super::{WorkspaceView, shared::SIDEBAR_WIDTH};

impl WorkspaceView {
    pub(super) fn sidebar(&self, cx: &mut Context<Self>) -> Div {
        div()
            .w(px(SIDEBAR_WIDTH))
            .min_h_0()
            .bg(theme::bg_sidebar())
            .border_1()
            .border_color(theme::border_base())
            .rounded(px(26.0))
            .shadow_xs()
            .child(
                div().v_flex().h_full().p(px(14.0)).child(
                    div()
                        .v_flex()
                        .h_full()
                        .min_h_0()
                        .gap(px(12.0))
                        .p(px(12.0))
                        .bg(theme::bg_console())
                        .border_1()
                        .border_color(theme::border_soft())
                        .rounded(px(22.0))
                        .child(
                            div()
                                .v_flex()
                                .gap(px(6.0))
                                .child(
                                    div()
                                        .text_size(px(9.0))
                                        .font_semibold()
                                        .text_color(theme::accent_cyan())
                                        .child("NAVIGATION"),
                                )
                                .child(
                                    div()
                                        .text_size(px(11.0))
                                        .text_color(theme::fg_muted())
                                        .line_clamp(2)
                                        .text_ellipsis()
                                        .child("Workspace routes only."),
                                ),
                        )
                        .child(div().h(px(1.0)).w_full().bg(theme::border_soft()))
                        .child(
                            div().flex_1().min_h_0().overflow_y_scrollbar().child(
                                div()
                                    .v_flex()
                                    .gap(px(10.0))
                                    .child(self.nav_item(
                                        "nav-home",
                                        "Home",
                                        WorkspaceRoute::Home,
                                        cx,
                                    ))
                                    .child(self.nav_item(
                                        "nav-clipboard",
                                        "Clipboard",
                                        WorkspaceRoute::Clipboard,
                                        cx,
                                    ))
                                    .child(self.nav_item(
                                        "nav-transfers",
                                        "Transfers",
                                        WorkspaceRoute::Transfers,
                                        cx,
                                    ))
                                    .child(self.nav_item(
                                        "nav-peers",
                                        "Peers",
                                        WorkspaceRoute::Peers,
                                        cx,
                                    ))
                                    .child(self.nav_item(
                                        "nav-settings",
                                        "Settings",
                                        WorkspaceRoute::Settings,
                                        cx,
                                    )),
                            ),
                        ),
                ),
            )
    }
}
