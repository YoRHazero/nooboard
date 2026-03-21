use gpui::Context;

use crate::{ui::theme, workspace::view_state::SettingsPageViewState};

use super::super::WorkspaceView;
use super::state::SettingsSectionKey;

impl WorkspaceView {
    pub(super) fn settings_banner_status(
        &self,
        any_dirty: bool,
        applying: Option<SettingsSectionKey>,
    ) -> (&'static str, gpui::Hsla) {
        if applying.is_some() {
            ("Applying", theme::accent_cyan())
        } else if any_dirty {
            ("Draft", theme::accent_amber())
        } else {
            ("Current", theme::accent_green())
        }
    }

    pub(super) fn settings_section_status(
        &self,
        key: SettingsSectionKey,
        dirty: bool,
    ) -> (&'static str, gpui::Hsla) {
        if self.settings.applying() == Some(key) {
            ("Applying", theme::accent_cyan())
        } else if dirty {
            ("Draft", theme::accent_amber())
        } else {
            ("Current", theme::accent_green())
        }
    }

    pub(super) fn connection_settings_dirty(
        &self,
        state: &SettingsPageViewState,
        cx: &Context<Self>,
    ) -> bool {
        self.settings.device_id_value(cx).trim() != state.connection.device_id
            || self.settings.token_value(cx).trim() != state.connection.token
            || self.settings.listen_port_value(cx).trim()
                != state.connection.listen_port.to_string()
            || self.settings.lan_enabled() != state.connection.lan_enabled
    }

    pub(super) fn clipboard_settings_dirty(&self, state: &SettingsPageViewState) -> bool {
        self.settings.local_capture_enabled() != state.clipboard.local_capture_enabled
    }

    pub(super) fn transfer_settings_dirty(
        &self,
        state: &SettingsPageViewState,
        cx: &Context<Self>,
    ) -> bool {
        self.settings.download_dir_value(cx).trim()
            != state.transfers.download_dir.display().to_string()
    }

    pub(super) fn storage_settings_dirty(
        &self,
        state: &SettingsPageViewState,
        cx: &Context<Self>,
    ) -> bool {
        self.settings
            .history_window_days(cx)
            .map_or(true, |value| value != state.storage.history_window_days)
            || self
                .settings
                .dedup_window_days(cx)
                .map_or(true, |value| value != state.storage.dedup_window_days)
            || self
                .settings
                .max_text_bytes(cx)
                .map_or(true, |value| value != state.storage.max_text_bytes)
    }
}
