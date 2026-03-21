use gpui::{Context, IntoElement, ParentElement, Styled, div, px};
use gpui_component::StyledExt;

use crate::{ui::theme, workspace::view_state::SettingsPageViewState};

use super::super::super::WorkspaceView;
use super::super::state::SettingsSectionKey;

impl WorkspaceView {
    pub(in crate::ui::workspace::settings) fn clipboard_settings_panel(
        &self,
        state: &SettingsPageViewState,
        dirty: bool,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let (status_label, status_accent) =
            self.settings_section_status(SettingsSectionKey::Clipboard, dirty);
        let actions_enabled = dirty && self.settings.applying().is_none();

        self.settings_section_shell(
            "Clipboard",
            "Choose whether text copied on this device is shared automatically.",
            self.settings_status_chip(status_label, status_accent),
        )
        .child(
            div()
                .v_flex()
                .gap(px(8.0))
                .child(
                    div()
                        .h_flex()
                        .items_center()
                        .justify_between()
                        .gap(px(12.0))
                        .px(px(14.0))
                        .py(px(12.0))
                        .bg(theme::bg_console())
                        .border_1()
                        .border_color(if self.settings.local_capture_enabled() {
                            theme::accent_blue().opacity(0.34)
                        } else {
                            theme::border_soft()
                        })
                        .rounded(px(18.0))
                        .child(
                            div()
                                .v_flex()
                                .gap(px(4.0))
                                .child(
                                    div()
                                        .text_size(px(12.0))
                                        .font_semibold()
                                        .text_color(theme::fg_primary())
                                        .child("Share Local Clipboard"),
                                )
                                .child(
                                    div()
                                        .text_size(px(10.0))
                                        .text_color(theme::fg_muted())
                                        .line_clamp(2)
                                        .text_ellipsis()
                                        .child("When off, items copied on this device stay local."),
                                ),
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
                                        .text_color(if self.settings.local_capture_enabled() {
                                            theme::accent_blue()
                                        } else {
                                            theme::fg_muted()
                                        })
                                        .child(if self.settings.local_capture_enabled() {
                                            "On"
                                        } else {
                                            "Off"
                                        }),
                                )
                                .child(self.settings_inline_switch(
                                    "settings-toggle-local-capture",
                                    self.settings.local_capture_enabled(),
                                    theme::accent_blue(),
                                    cx.listener(|this, _, _, cx| {
                                        this.request_settings_toggle_local_capture(cx);
                                    }),
                                )),
                        ),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(theme::fg_muted())
                        .child(format!(
                            "Saved setting: {}",
                            if state.clipboard.local_capture_enabled {
                                "On"
                            } else {
                                "Off"
                            }
                        )),
                ),
        )
        .child(
            div()
                .h_flex()
                .justify_end()
                .gap(px(8.0))
                .child(self.settings_compact_action_button(
                    "settings-reset-clipboard",
                    "Reset",
                    "Undo the changes in this section."
                        .to_string(),
                    actions_enabled,
                    theme::accent_rose(),
                    |this, _, _, cx| {
                        this.request_reset_clipboard_settings(cx);
                    },
                    cx,
                ))
                .child(self.settings_compact_action_button(
                    "settings-apply-clipboard",
                    "Apply",
                    "Save the changes in this section.".to_string(),
                    actions_enabled,
                    theme::accent_blue(),
                    |this, _, _, cx| {
                        this.request_apply_clipboard_settings(cx);
                    },
                    cx,
                )),
        )
    }
}
