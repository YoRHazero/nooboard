use gpui::{
    Context, Div, Hsla, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement,
    Styled, div, px,
};
use gpui_component::Disableable;
use gpui_component::StyledExt;

use crate::ui::theme;

use super::components::{
    settings_action_button, settings_action_row, settings_button_with_tooltip,
    settings_section_footer, settings_section_shell, settings_status_chip,
};
use super::WorkspaceView;

#[derive(Clone, Copy)]
enum ToggleKind {
    NetworkEnabled,
    MdnsEnabled,
}

impl WorkspaceView {
    pub(super) fn network_settings_panel(&self, cx: &mut Context<Self>) -> Div {
        let draft = self.network_settings_draft();
        let dirty = self.network_settings_dirty();
        let patch_labels = self.network_patch_labels();
        let status = if dirty {
            settings_status_chip("Modified", theme::accent_amber())
        } else {
            settings_status_chip("Current", theme::accent_green())
        };

        settings_section_shell(
            "Network",
            "Adjust the draft settings for networking and local discovery.",
            status,
        )
            .child(self.network_toggle_row(
                "Network service",
                draft.network_enabled,
                self.network_settings_confirmed().network_enabled,
                theme::accent_cyan(),
                ToggleKind::NetworkEnabled,
                cx,
            ))
            .child(self.network_toggle_row(
                "Local discovery (mDNS)",
                draft.mdns_enabled,
                self.network_settings_confirmed().mdns_enabled,
                theme::accent_blue(),
                ToggleKind::MdnsEnabled,
                cx,
            ))
            .child(settings_section_footer(
                if patch_labels.is_empty() {
                    "No network changes ready for review.".to_string()
                } else {
                    format!("Changed: {}", patch_labels.join(", "))
                },
                theme::fg_muted(),
                settings_action_row([
                    settings_action_button(
                        "settings-reset-network-draft",
                        "Reset",
                        theme::accent_rose(),
                        cx,
                    )
                    .disabled(!dirty)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.reset_network_settings_draft(cx);
                    }))
                    .into_any_element(),
                    settings_button_with_tooltip(
                        "settings-review-network-tooltip",
                        settings_action_button(
                            "settings-stage-network-patch",
                            "Review",
                            theme::accent_blue(),
                            cx,
                        )
                        .disabled(!dirty)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.stage_network_patch(cx);
                        })),
                        Some(
                            "Preview which network settings would be prepared from this draft. No backend call happens yet.",
                        ),
                    ),
                ]),
            ))
    }

    fn network_toggle_row(
        &self,
        label: &'static str,
        enabled: bool,
        confirmed_enabled: bool,
        accent: Hsla,
        toggle_kind: ToggleKind,
        cx: &mut Context<Self>,
    ) -> Div {
        let matches_confirmed = enabled == confirmed_enabled;

        div()
            .h_flex()
            .items_start()
            .justify_between()
            .gap(px(12.0))
            .child(
                div()
                    .v_flex()
                    .gap(px(4.0))
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(theme::fg_secondary())
                            .child(label.to_string()),
                    )
                    .child(
                        div()
                            .text_size(px(10.0))
                            .text_color(if matches_confirmed {
                                theme::fg_muted()
                            } else {
                                accent
                            })
                            .child(if matches_confirmed {
                                "Matches the current setting".to_string()
                            } else {
                                format!(
                                    "Current setting: {}",
                                    if confirmed_enabled { "on" } else { "off" }
                                )
                            }),
                    ),
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
            .child(div().size(px(6.0)).rounded(px(999.0)).bg(if enabled {
                accent
            } else {
                theme::fg_muted()
            }))
            .child(
                div()
                    .text_size(px(10.0))
                    .font_semibold()
                    .text_color(if enabled { accent } else { theme::fg_muted() })
                    .child(if enabled { "on" } else { "off" }),
            )
    }
}
