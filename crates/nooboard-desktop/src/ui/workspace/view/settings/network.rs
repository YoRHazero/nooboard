use gpui::{
    Context, Div, Hsla, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement,
    Styled, div, px,
};
use gpui_component::StyledExt;

use crate::ui::theme;

use super::WorkspaceView;

#[derive(Clone, Copy)]
enum ToggleKind {
    NetworkEnabled,
    MdnsEnabled,
}

impl WorkspaceView {
    pub(super) fn network_settings_panel(&self, cx: &mut Context<Self>) -> Div {
        div()
            .flex_1()
            .min_w(px(0.0))
            .v_flex()
            .gap(px(14.0))
            .p(px(18.0))
            .bg(theme::bg_panel())
            .border_1()
            .border_color(theme::border_base())
            .rounded(px(22.0))
            .shadow_xs()
            .child(
                div()
                    .text_size(px(18.0))
                    .font_semibold()
                    .text_color(theme::fg_primary())
                    .child("Network"),
            )
            .child(div().h(px(1.0)).w_full().bg(theme::border_soft()))
            .child(self.network_toggle_row(
                "Network enabled",
                self.settings_page_state.network_enabled,
                theme::accent_cyan(),
                ToggleKind::NetworkEnabled,
                cx,
            ))
            .child(self.network_toggle_row(
                "mDNS enabled",
                self.settings_page_state.mdns_enabled,
                theme::accent_blue(),
                ToggleKind::MdnsEnabled,
                cx,
            ))
            .child(
                div()
                    .pt(px(8.0))
                    .child(
                        self.settings_action_button(
                            "settings-save-network-patch",
                            "Save Network Patch",
                            theme::accent_blue(),
                            cx,
                        )
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.save_network_patch(cx);
                            })),
                    ),
            )
    }

    fn network_toggle_row(
        &self,
        label: &'static str,
        enabled: bool,
        accent: Hsla,
        toggle_kind: ToggleKind,
        cx: &mut Context<Self>,
    ) -> Div {
        div()
            .h_flex()
            .items_center()
            .justify_between()
            .gap(px(12.0))
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(theme::fg_secondary())
                    .child(label.to_string()),
            )
            .child(self.network_toggle_chip(enabled, accent, toggle_kind, cx))
    }

    fn network_toggle_chip(
        &self,
        enabled: bool,
        accent: Hsla,
        toggle_kind: ToggleKind,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .id(match toggle_kind {
                ToggleKind::NetworkEnabled => "settings-toggle-network-enabled",
                ToggleKind::MdnsEnabled => "settings-toggle-mdns-enabled",
            })
            .h_flex()
            .items_center()
            .gap(px(8.0))
            .px(px(10.0))
            .py(px(5.0))
            .cursor_pointer()
            .bg(if enabled {
                accent.opacity(0.14)
            } else {
                theme::bg_console()
            })
            .border_1()
            .border_color(if enabled {
                accent.opacity(0.32)
            } else {
                theme::border_soft()
            })
            .rounded(px(999.0))
            .hover(|this| {
                this.bg(theme::bg_panel_alt())
                    .border_color(theme::border_strong())
            })
            .active(|this| this.bg(theme::bg_panel()))
            .on_click(cx.listener(move |this, _, _, cx| match toggle_kind {
                ToggleKind::NetworkEnabled => this.toggle_settings_network_enabled(cx),
                ToggleKind::MdnsEnabled => this.toggle_settings_mdns_enabled(cx),
            }))
            .child(
                div()
                    .size(px(6.0))
                    .rounded(px(999.0))
                    .bg(if enabled { accent } else { theme::fg_muted() }),
            )
            .child(
                div()
                    .text_size(px(10.0))
                    .font_semibold()
                    .text_color(if enabled { accent } else { theme::fg_muted() })
                    .child(if enabled { "on" } else { "off" }),
            )
    }
}
