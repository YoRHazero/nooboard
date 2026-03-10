use gpui::{
    Context, Hsla, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement,
    Styled, div, px,
};
use gpui_component::{Icon, IconName, StyledExt};

use crate::state::WorkspaceRoute;
use crate::ui::theme;

use super::WorkspaceView;

fn route_icon(route: WorkspaceRoute) -> IconName {
    match route {
        WorkspaceRoute::Home => IconName::LayoutDashboard,
        WorkspaceRoute::Clipboard => IconName::Copy,
        WorkspaceRoute::Peers => IconName::Globe,
        WorkspaceRoute::Transfers => IconName::Folder,
        WorkspaceRoute::Settings => IconName::Settings2,
    }
}

fn route_subtitle(route: WorkspaceRoute) -> &'static str {
    match route {
        WorkspaceRoute::Home => "runtime overview",
        WorkspaceRoute::Clipboard => "broadcast console",
        WorkspaceRoute::Peers => "mesh topology",
        WorkspaceRoute::Transfers => "file lanes",
        WorkspaceRoute::Settings => "system tuning",
    }
}

fn route_code(route: WorkspaceRoute) -> &'static str {
    match route {
        WorkspaceRoute::Home => "01",
        WorkspaceRoute::Clipboard => "02",
        WorkspaceRoute::Transfers => "03",
        WorkspaceRoute::Peers => "04",
        WorkspaceRoute::Settings => "05",
    }
}

fn route_accent(route: WorkspaceRoute) -> Hsla {
    match route {
        WorkspaceRoute::Home => theme::accent_cyan(),
        WorkspaceRoute::Clipboard => theme::accent_blue(),
        WorkspaceRoute::Peers => theme::accent_cyan(),
        WorkspaceRoute::Transfers => theme::accent_amber(),
        WorkspaceRoute::Settings => theme::accent_rose(),
    }
}

impl WorkspaceView {
    pub(super) fn nav_item(
        &self,
        id: &'static str,
        label: &'static str,
        route: WorkspaceRoute,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let active = self.route == route;
        let accent = route_accent(route);
        let icon = route_icon(route);
        let subtitle = route_subtitle(route);

        div()
            .id(id)
            .w_full()
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, window, cx| {
                this.request_workspace_route(route, window, cx);
                cx.notify();
            }))
            .px(px(14.0))
            .py(px(12.0))
            .bg(if active {
                theme::bg_panel_highlight()
            } else {
                theme::bg_console()
            })
            .border_1()
            .border_color(if active {
                theme::border_strong()
            } else {
                theme::border_soft()
            })
            .rounded(px(20.0))
            .shadow_xs()
            .hover(|this| {
                this.bg(theme::bg_panel_alt())
                    .border_color(theme::border_strong())
            })
            .active(|this| this.bg(theme::bg_panel()))
            .child(
                div()
                    .v_flex()
                    .gap(px(8.0))
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
                                    .gap(px(10.0))
                                    .child(
                                        div()
                                            .size(px(30.0))
                                            .rounded(px(11.0))
                                            .bg(if active {
                                                accent.opacity(0.14)
                                            } else {
                                                theme::bg_panel()
                                            })
                                            .border_1()
                                            .border_color(if active {
                                                accent.opacity(0.32)
                                            } else {
                                                theme::border_base()
                                            })
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .child(Icon::new(icon).size(px(15.0)).text_color(
                                                if active {
                                                    accent
                                                } else {
                                                    theme::fg_secondary()
                                                },
                                            )),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w(px(0.0))
                                            .h_flex()
                                            .items_center()
                                            .gap(px(8.0))
                                            .child(
                                                div()
                                                    .text_size(px(12.0))
                                                    .font_semibold()
                                                    .text_color(if active {
                                                        theme::fg_primary()
                                                    } else {
                                                        theme::fg_secondary()
                                                    })
                                                    .line_clamp(1)
                                                    .text_ellipsis()
                                                    .child(label.to_string()),
                                            )
                                            .child(div().size(px(6.0)).rounded(px(999.0)).bg(
                                                if active { accent } else { theme::border_base() },
                                            )),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(10.0))
                                    .font_semibold()
                                    .text_color(if active { accent } else { theme::fg_muted() })
                                    .child(route_code(route)),
                            ),
                    )
                    .child(
                        div()
                            .pl(px(40.0))
                            .pr(px(4.0))
                            .text_size(px(10.0))
                            .text_color(theme::fg_muted())
                            .line_clamp(2)
                            .text_ellipsis()
                            .child(subtitle.to_string()),
                    )
                    .child(
                        div()
                            .h(px(2.0))
                            .w_full()
                            .bg(if active {
                                accent.opacity(0.95)
                            } else {
                                theme::border_soft()
                            })
                            .rounded(px(999.0)),
                    ),
            )
    }
}
