use gpui::{
    Context, Div, Hsla, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement,
    Styled, div, px,
};
use gpui_component::Disableable;
use gpui_component::StyledExt;

use crate::ui::theme;

use super::WorkspaceView;
use super::components::{
    settings_action_button, settings_action_row, settings_section_footer, settings_section_shell,
    settings_status_chip,
};
use super::{SettingsSectionPhase, settings_status_tokens};

impl WorkspaceView {
    pub(super) fn clipboard_settings_panel(&self, cx: &mut Context<Self>) -> Div {
        let dirty = !self.clipboard_patch_labels().is_empty();
        let (status_label, status_accent) =
            settings_status_tokens(self.clipboard_settings_status());
        let status = settings_status_chip(status_label, status_accent);

        settings_section_shell(
            "Clipboard",
            "Configure whether the app captures local clipboard changes into committed history.",
            status,
        )
        .child(self.clipboard_toggle_row(
            "Local clipboard capture",
            self.clipboard_settings_draft().local_capture_enabled,
            self.clipboard_settings_confirmed().local_capture_enabled,
            theme::accent_cyan(),
            cx,
        ))
        .child(settings_section_footer(
            if self.clipboard_patch_labels().is_empty() {
                "Clipboard settings match the current app state.".to_string()
            } else if let Some(message) = Self::settings_phase_summary(
                self.clipboard_settings_phase(),
                "Live clipboard settings changed while this draft was open.",
            ) {
                message
            } else {
                format!("Changed: {}", self.clipboard_patch_labels().join(", "))
            },
            theme::fg_muted(),
            settings_action_row([
                settings_action_button(
                    "settings-reset-clipboard-draft",
                    "Reset",
                    theme::accent_rose(),
                    cx,
                )
                .disabled(!dirty)
                .on_click(cx.listener(|this, _, _, cx| {
                    this.reset_clipboard_settings_draft(cx);
                }))
                .into_any_element(),
                settings_action_button(
                    "settings-apply-clipboard-patch",
                    "Apply",
                    theme::accent_cyan(),
                    cx,
                )
                .disabled(
                    !dirty
                        || matches!(
                            self.clipboard_settings_phase(),
                            SettingsSectionPhase::Applying
                        ),
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.apply_clipboard_settings(cx);
                }))
                .into_any_element(),
            ]),
        ))
    }

    fn clipboard_toggle_row(
        &self,
        label: &'static str,
        enabled: bool,
        confirmed_enabled: bool,
        accent: Hsla,
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
                            .text_color(theme::fg_muted())
                            .line_clamp(2)
                            .text_ellipsis()
                            .child(
                                "When disabled, local clipboard updates stop entering app history.",
                            ),
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
            .child(
                div()
                    .id("settings-toggle-local-capture-enabled")
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
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.toggle_settings_local_capture_enabled(cx);
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
                    ),
            )
    }
}
