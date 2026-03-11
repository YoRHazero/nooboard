mod apply;
mod clipboard;
mod components;
mod derived;
mod draft_ops;
mod header;
mod network;
mod page_state;
mod patches;
mod section_state;
mod snapshot;
mod storage;
mod transfers;

use gpui::{Context, Div, ParentElement, Styled, Window, div, px};
use gpui_component::StyledExt;

use self::components::settings_feedback_banner;
use crate::ui::theme;

use super::WorkspaceView;

pub(super) use page_state::SettingsPageState;
pub(super) use section_state::{
    SettingsSection, SettingsSectionPhase, SettingsStatus, StorageSettingField,
    settings_section_status,
};
pub(super) use snapshot::build_settings_snapshot;

pub(super) fn settings_status_tokens(status: SettingsStatus) -> (&'static str, gpui::Hsla) {
    match status {
        SettingsStatus::Current => ("Current", theme::accent_green()),
        SettingsStatus::Modified => ("Modified", theme::accent_amber()),
        SettingsStatus::Applying => ("Applying", theme::accent_cyan()),
        SettingsStatus::Review => ("Review", theme::accent_rose()),
        SettingsStatus::Error => ("Error", theme::accent_rose()),
        SettingsStatus::Stale => ("Stale", theme::accent_blue()),
    }
}

impl WorkspaceView {
    pub(super) fn settings_page(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        self.settings_page_state.sync_network_inputs(window, cx);
        let (label, accent) = settings_status_tokens(self.settings_status());

        div()
            .w_full()
            .v_flex()
            .gap(px(18.0))
            .child(self.settings_header())
            .child(
                div()
                    .w_full()
                    .v_flex()
                    .gap(px(18.0))
                    .child(self.network_settings_panel(cx))
                    .child(self.clipboard_settings_panel(cx))
                    .child(self.transfer_settings_panel(cx))
                    .child(self.storage_settings_panel(cx)),
            )
            .child(settings_feedback_banner(
                label,
                accent,
                self.settings_status_message(),
            ))
    }
}
