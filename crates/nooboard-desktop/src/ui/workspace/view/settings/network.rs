use gpui::{
    Context, Div, Hsla, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement,
    Styled, div, px,
};
use gpui_component::Disableable;
use gpui_component::input::Input;
use gpui_component::{Sizable, StyledExt};

use crate::ui::theme;

use super::WorkspaceView;
use super::components::{
    settings_action_button, settings_action_row, settings_control_button, settings_section_footer,
    settings_section_shell, settings_status_chip,
};
use super::{SettingsSectionPhase, settings_status_tokens};

#[derive(Clone, Copy)]
enum ToggleKind {
    NetworkEnabled,
    MdnsEnabled,
}

impl WorkspaceView {
    pub(super) fn network_settings_panel(&self, cx: &mut Context<Self>) -> Div {
        let draft = self.network_settings_draft();
        let dirty = !self.network_patch_labels().is_empty();
        let issues = self.network_validation_issues();
        let patch_labels = self.network_patch_labels();
        let (status_label, status_accent) = settings_status_tokens(self.network_settings_status());
        let status = settings_status_chip(status_label, status_accent);

        settings_section_shell(
            "Network",
            "Control sync networking, mDNS discovery, and manual peer endpoints.",
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
        .child(self.manual_peer_editor(cx))
        .child(settings_section_footer(
            if !issues.is_empty() {
                format!("Validation: {}", issues.join("; "))
            } else if patch_labels.is_empty() {
                "Network settings match the current app state.".to_string()
            } else if let Some(message) = Self::settings_phase_summary(
                self.network_settings_phase(),
                "Live network settings changed while this draft was open.",
            ) {
                message
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
                settings_action_button(
                    "settings-apply-network-patch",
                    "Apply",
                    theme::accent_blue(),
                    cx,
                )
                .disabled(
                    !dirty
                        || !issues.is_empty()
                        || matches!(
                            self.network_settings_phase(),
                            SettingsSectionPhase::Applying
                        ),
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.apply_network_settings(cx);
                }))
                .into_any_element(),
            ]),
        ))
    }

    fn manual_peer_editor(&self, cx: &mut Context<Self>) -> Div {
        let current = self
            .network_settings_confirmed()
            .manual_peers
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        let dirty = self.network_settings_draft().manual_peers
            != self.network_settings_confirmed().manual_peers;
        let peer_rows = self
            .network_settings_draft()
            .manual_peers
            .iter()
            .map(|addr| {
                let addr_value = *addr;

                div()
                    .h_flex()
                    .items_center()
                    .justify_between()
                    .gap(px(8.0))
                    .px(px(12.0))
                    .py(px(9.0))
                    .bg(theme::bg_console())
                    .border_1()
                    .border_color(theme::border_soft())
                    .rounded(px(14.0))
                    .child(
                        div()
                            .min_w(px(0.0))
                            .text_size(px(12.0))
                            .text_color(theme::fg_primary())
                            .line_clamp(1)
                            .text_ellipsis()
                            .child(addr.to_string()),
                    )
                    .child(
                        settings_control_button(
                            format!("settings-remove-manual-peer-{addr}"),
                            "×",
                            theme::accent_rose(),
                            cx,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.remove_settings_manual_peer(addr_value, cx);
                        })),
                    )
            })
            .collect::<Vec<_>>();

        div()
            .v_flex()
            .gap(px(10.0))
            .child(
                div()
                    .v_flex()
                    .gap(px(4.0))
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(theme::fg_secondary())
                            .child("Manual peers"),
                    )
                    .child(
                        div()
                            .text_size(px(10.0))
                            .text_color(theme::fg_muted())
                            .line_clamp(2)
                            .text_ellipsis()
                            .child("Add direct IP:port peers when local discovery is unavailable."),
                    )
                    .child(
                        div()
                            .text_size(px(10.0))
                            .text_color(if dirty {
                                theme::accent_amber()
                            } else {
                                theme::fg_muted()
                            })
                            .line_clamp(2)
                            .text_ellipsis()
                            .child(if dirty {
                                format!(
                                    "Current peers: {}",
                                    if current.is_empty() {
                                        "none".to_string()
                                    } else {
                                        current.join(", ")
                                    }
                                )
                            } else {
                                "Matches the current peer list".to_string()
                            }),
                    ),
            )
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        Input::new(&self.settings_page_state.manual_peer_input)
                            .small()
                            .flex_1(),
                    )
                    .child(
                        settings_action_button(
                            "settings-add-manual-peer",
                            "Add",
                            theme::accent_cyan(),
                            cx,
                        )
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.commit_settings_manual_peer(window, cx);
                        })),
                    ),
            )
            .children(if peer_rows.is_empty() {
                vec![
                    div()
                        .text_size(px(11.0))
                        .text_color(theme::fg_muted())
                        .child("No manual peers in this draft.")
                        .into_any_element(),
                ]
            } else {
                peer_rows
                    .into_iter()
                    .map(IntoElement::into_any_element)
                    .collect()
            })
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
