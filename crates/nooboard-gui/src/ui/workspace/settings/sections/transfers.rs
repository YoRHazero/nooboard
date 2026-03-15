use gpui::{Context, IntoElement, ParentElement, Styled, div};
use gpui_component::input::Input;
use gpui_component::{Disableable, Sizable, StyledExt};

use crate::{ui::theme, workspace::view_state::SettingsPageViewState};

use super::super::super::WorkspaceView;
use super::super::state::SettingsSectionKey;

impl WorkspaceView {
    pub(in crate::ui::workspace::settings) fn transfer_settings_panel(
        &self,
        state: &SettingsPageViewState,
        dirty: bool,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let (status_label, status_accent) =
            self.settings_section_status(SettingsSectionKey::Transfers, dirty);

        self.settings_section_shell(
            "Transfers",
            "Incoming files are saved into the configured download directory.",
            self.settings_status_chip(status_label, status_accent),
        )
        .child(
            self.settings_input_field(
                "Download Directory",
                "Changed locally first, then persisted through the transfer settings command.",
                Input::new(&self.settings.download_dir_input())
                    .small()
                    .appearance(false)
                    .bordered(false)
                    .focus_bordered(false)
                    .w_full(),
            ),
        )
        .child(self.settings_path_summary(&state.transfers))
        .child(
            div()
                .h_flex()
                .justify_end()
                .gap(gpui::px(8.0))
                .child(
                    self.settings_action_button(
                        "settings-browse-download-dir",
                        "Browse",
                        theme::accent_amber(),
                        cx,
                    )
                    .disabled(self.settings.applying().is_some())
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.pick_settings_download_dir(window, cx);
                    })),
                )
                .child(
                    self.settings_action_button(
                        "settings-apply-transfers",
                        "Apply Transfers",
                        theme::accent_cyan(),
                        cx,
                    )
                    .disabled(!dirty || self.settings.applying().is_some())
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.request_apply_transfer_settings(cx);
                    })),
                ),
        )
    }
}
