use gpui::{Context, IntoElement, ParentElement, Styled, div};
use gpui_component::{Disableable, StyledExt};

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

        self.settings_section_shell(
            "Clipboard",
            "Clipboard capture is local draft state until applied; the active switch comes from WorkspaceSnapshot.",
            self.settings_status_chip(status_label, status_accent),
        )
        .child(self.settings_toggle_chip(
            "settings-toggle-local-capture",
            "Local Capture",
            self.settings.local_capture_enabled(),
            theme::accent_blue(),
            "When disabled, local clipboard changes stay out of the sync graph.",
            cx.listener(|this, _, _, cx| {
                this.request_settings_toggle_local_capture(cx);
            }),
        ))
        .child(
            div()
                .h_flex()
                .items_center()
                .justify_between()
                .gap(gpui::px(12.0))
                .child(
                    div()
                        .text_size(gpui::px(11.0))
                        .text_color(theme::fg_muted())
                        .child(format!(
                            "Snapshot state: {}",
                            if state.clipboard.local_capture_enabled {
                                "enabled"
                            } else {
                                "disabled"
                            }
                        )),
                )
                .child(
                    self.settings_action_button(
                        "settings-apply-clipboard",
                        "Apply Clipboard",
                        theme::accent_blue(),
                        cx,
                    )
                    .disabled(!dirty || self.settings.applying().is_some())
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.request_apply_clipboard_settings(cx);
                    })),
                ),
        )
    }
}
