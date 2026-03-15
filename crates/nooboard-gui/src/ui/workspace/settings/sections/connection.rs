use gpui::{Context, IntoElement, ParentElement, Styled, div};
use gpui_component::input::Input;
use gpui_component::{Disableable, Sizable, StyledExt};

use crate::{ui::theme, workspace::view_state::SettingsPageViewState};

use super::super::super::WorkspaceView;
use super::super::state::SettingsSectionKey;

impl WorkspaceView {
    pub(in crate::ui::workspace::settings) fn connection_settings_panel(
        &self,
        state: &SettingsPageViewState,
        dirty: bool,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let (status_label, status_accent) =
            self.settings_section_status(SettingsSectionKey::Connection, dirty);

        self.settings_section_shell(
            "Connection",
            "Device identity, network token, and listen endpoint are applied directly through nooboard-core.",
            self.settings_status_chip(status_label, status_accent),
        )
        .child(
            div()
                .h_flex()
                .flex_wrap()
                .gap(gpui::px(12.0))
                .child(self.settings_input_field(
                    "Device ID",
                    "Shown to peers and rendered in the shell header.",
                    Input::new(&self.settings.device_id_input())
                        .small()
                        .appearance(false)
                        .bordered(false)
                        .focus_bordered(false)
                        .w_full(),
                ))
                .child(self.settings_input_field(
                    "Network Token",
                    "Shared token required for authenticated peers.",
                    Input::new(&self.settings.token_input())
                        .small()
                        .appearance(false)
                        .bordered(false)
                        .focus_bordered(false)
                        .w_full(),
                ))
                .child(self.settings_input_field(
                    "Listen Port",
                    "LAN discovery and direct-connect binds use this port.",
                    Input::new(&self.settings.listen_port_input())
                        .small()
                        .appearance(false)
                        .bordered(false)
                        .focus_bordered(false)
                        .w_full(),
                ))
                .child(self.settings_toggle_chip(
                    "settings-toggle-lan-enabled",
                    "LAN Discovery",
                    self.settings.lan_enabled(),
                    theme::accent_green(),
                    "Advertise this node on the local network.",
                    cx.listener(|this, _, _, cx| {
                        this.request_settings_toggle_lan_enabled(cx);
                    }),
                )),
        )
        .child(
            div()
                .h_flex()
                .flex_wrap()
                .items_center()
                .justify_between()
                .gap(gpui::px(12.0))
                .child(
                    div()
                        .h_flex()
                        .flex_wrap()
                        .gap(gpui::px(8.0))
                        .child(self.settings_meta_chip(
                            "Endpoint",
                            state
                                .connection
                                .endpoint_label
                                .as_deref()
                                .unwrap_or("not bound"),
                            theme::accent_cyan(),
                        ))
                        .child(self.settings_meta_chip(
                            "Current Token",
                            if state.connection.token.is_empty() {
                                "unset"
                            } else {
                                "configured"
                            },
                            theme::accent_blue(),
                        )),
                )
                .child(
                    self.settings_action_button(
                        "settings-apply-connection",
                        "Apply Connection",
                        theme::accent_cyan(),
                        cx,
                    )
                    .disabled(!dirty || self.settings.applying().is_some())
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.request_apply_connection_settings(cx);
                    })),
                ),
        )
    }
}
