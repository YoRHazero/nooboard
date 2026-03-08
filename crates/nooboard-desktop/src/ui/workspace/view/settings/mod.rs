mod components;
mod header;
mod network;
mod page_state;
mod storage;

use gpui::{Context, Div, ParentElement, Styled, div, px};
use gpui_component::StyledExt;

use self::components::settings_feedback_banner;
use crate::ui::theme;

use super::WorkspaceView;

pub(super) use page_state::{SettingsPageState, SettingsSaveState, StorageSettingField};

impl WorkspaceView {
    pub(super) fn settings_page(&self, cx: &mut Context<Self>) -> Div {
        let dirty_fields = self.settings_dirty_field_count();
        let has_validation_issues = !self.storage_validation_issues().is_empty();
        let (label, accent, message) = match self.settings_save_state() {
            SettingsSaveState::Ready => (
                "Preview",
                theme::accent_cyan(),
                self.settings_feedback()
                    .unwrap_or("Draft changes are ready for review.")
                    .to_string(),
            ),
            SettingsSaveState::Invalid => (
                "Review",
                theme::accent_rose(),
                self.settings_feedback()
                    .unwrap_or("Resolve the highlighted storage issues first.")
                    .to_string(),
            ),
            SettingsSaveState::Idle if dirty_fields > 0 => (
                "Draft",
                theme::accent_amber(),
                self.settings_feedback()
                    .unwrap_or("Draft values differ from the current settings.")
                    .to_string(),
            ),
            _ => (
                "Current",
                theme::accent_green(),
                if has_validation_issues {
                    "Resolve the highlighted storage issues first.".to_string()
                } else {
                    "Draft values match the current settings.".to_string()
                },
            ),
        };

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
                    .child(self.storage_settings_panel(cx))
                    .child(self.network_settings_panel(cx)),
            )
            .child(settings_feedback_banner(label, accent, message))
    }
}
