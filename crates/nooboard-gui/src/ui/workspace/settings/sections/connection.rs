use gpui::{
    Context, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled,
    div, px,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::Input;
use gpui_component::{IconName, Sizable, StyledExt};

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
        let endpoint_label = state
            .connection
            .endpoint_label
            .as_deref()
            .unwrap_or("Not available yet");
        let token_masked = self.settings.token_masked();
        let actions_enabled = dirty && self.settings.applying().is_none();

        self.settings_section_shell(
            "Connection",
            "Choose the name other devices see, the shared token they use to connect, and the port nooboard listens on.",
            self.settings_status_chip(status_label, status_accent),
        )
        .child(
            div()
                .v_flex()
                .gap(px(12.0))
                .child(
                    div()
                        .h_flex()
                        .flex_wrap()
                        .gap(px(12.0))
                        .child(self.settings_input_field_with_tooltip(
                    "settings-connection-device-id",
                    "Device ID",
                    "Other devices will see this name when they connect.",
                    Input::new(&self.settings.device_id_input())
                        .small()
                        .appearance(false)
                        .bordered(false)
                        .focus_bordered(false)
                        .w_full(),
                ))
                .child(self.settings_input_field_with_tooltip(
                    "settings-connection-token",
                    "Network Token",
                    "Devices need the same token to connect to each other.",
                    Input::new(&self.settings.token_input())
                        .small()
                        .appearance(false)
                        .bordered(false)
                        .focus_bordered(false)
                        .suffix(
                            div()
                                .id("settings-connection-token-visibility-shell")
                                .tooltip({
                                    let text = if token_masked {
                                        "Show network token".to_string()
                                    } else {
                                        "Hide network token".to_string()
                                    };
                                    move |window, cx| {
                                        Self::settings_themed_tooltip(
                                            text.clone(),
                                            window,
                                            cx,
                                        )
                                    }
                                })
                                .child(
                                    Button::new("settings-connection-token-visibility")
                                        .ghost()
                                        .xsmall()
                                        .icon(if token_masked {
                                            IconName::Eye
                                        } else {
                                            IconName::EyeOff
                                        })
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.request_settings_toggle_connection_token_mask(
                                                window, cx,
                                            );
                                        })),
                                ),
                        )
                        .w_full(),
                ))
                .child(self.settings_input_field_with_tooltip(
                    "settings-connection-port",
                    "Listen Port",
                    "Nearby discovery and direct connections use this port.",
                    Input::new(&self.settings.listen_port_input())
                        .small()
                        .appearance(false)
                        .bordered(false)
                        .focus_bordered(false)
                        .w_full(),
                )),
                )
                .child(
                    div()
                        .h_flex()
                        .flex_wrap()
                        .items_stretch()
                        .gap(px(12.0))
                        .child(
                            div()
                                .min_w(px(260.0))
                                .flex_1()
                                .v_flex()
                                .gap(px(8.0))
                                .p(px(14.0))
                                .bg(theme::bg_console())
                                .border_1()
                                .border_color(theme::border_soft())
                                .rounded(px(18.0))
                                .child(
                                    div()
                                        .h_flex()
                                        .items_center()
                                        .justify_between()
                                        .gap(px(12.0))
                                        .child(
                                            div()
                                                .text_size(px(12.0))
                                                .font_semibold()
                                                .text_color(theme::fg_primary())
                                                .child("Current Address"),
                                        )
                                        .child(self.settings_status_chip(
                                            "Current",
                                            theme::accent_cyan(),
                                        )),
                                )
                                .child(
                                    div()
                                        .text_size(px(11.0))
                                        .text_color(theme::fg_secondary())
                                        .line_clamp(2)
                                        .text_ellipsis()
                                        .child(endpoint_label.to_string()),
                                ),
                        )
                        .child(
                            div()
                                .min_w(px(260.0))
                                .flex_1()
                                .h_flex()
                                .items_center()
                                .justify_between()
                                .gap(px(12.0))
                                .px(px(14.0))
                                .py(px(12.0))
                                .bg(theme::bg_console())
                                .border_1()
                                .border_color(if self.settings.lan_enabled() {
                                    theme::accent_green().opacity(0.34)
                                } else {
                                    theme::border_soft()
                                })
                                .rounded(px(18.0))
                                .child(
                                    div()
                                        .h_flex()
                                        .items_center()
                                        .gap(px(8.0))
                                        .child(
                                            div()
                                                .text_size(px(12.0))
                                                .font_semibold()
                                                .text_color(theme::fg_primary())
                                                .child("Nearby Discovery"),
                                        )
                                        .child(self.settings_info_tooltip_icon(
                                            "settings-connection-lan",
                                            "Let devices on the same local network find this device automatically.".to_string(),
                                        )),
                                )
                                .child(
                                    div()
                                        .h_flex()
                                        .items_center()
                                        .gap(px(10.0))
                                        .child(
                                            div()
                                                .text_size(px(11.0))
                                                .font_semibold()
                                                .text_color(if self.settings.lan_enabled() {
                                                    theme::accent_green()
                                                } else {
                                                    theme::fg_muted()
                                                })
                                                .child(if self.settings.lan_enabled() {
                                                    "On"
                                                } else {
                                                    "Off"
                                                }),
                                        )
                                        .child(self.settings_inline_switch(
                                            "settings-toggle-lan-enabled",
                                            self.settings.lan_enabled(),
                                            theme::accent_green(),
                                            cx.listener(|this, _, _, cx| {
                                                this.request_settings_toggle_lan_enabled(cx);
                                            }),
                                        )),
                                ),
                        ),
                ),
        )
        .child(
            div()
                .h_flex()
                .justify_end()
                .gap(px(8.0))
                .child(self.settings_compact_action_button(
                    "settings-reset-connection",
                    "Reset",
                    "Undo the changes in this section."
                        .to_string(),
                    actions_enabled,
                    theme::accent_rose(),
                    |this, _, window, cx| {
                        this.request_reset_connection_settings(window, cx);
                    },
                    cx,
                ))
                .child(self.settings_compact_action_button(
                        "settings-apply-connection",
                        "Apply",
                        "Save the changes in this section."
                            .to_string(),
                        actions_enabled,
                        theme::accent_cyan(),
                        |this, _, _, cx| {
                        this.request_apply_connection_settings(cx);
                    },
                        cx,
                )),
        )
    }
}
