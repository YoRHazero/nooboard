use gpui::{Context, IntoElement, ParentElement, Styled, div};
use gpui_component::input::Input;
use gpui_component::{Disableable, Sizable, StyledExt};

use crate::{ui::theme, workspace::view_state::SettingsPageViewState};

use super::super::super::WorkspaceView;
use super::super::state::SettingsSectionKey;

impl WorkspaceView {
    pub(in crate::ui::workspace::settings) fn storage_settings_panel(
        &self,
        state: &SettingsPageViewState,
        dirty: bool,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let (status_label, status_accent) =
            self.settings_section_status(SettingsSectionKey::Storage, dirty);

        self.settings_section_shell(
            "Storage",
            "Retention and dedup controls are writable; db_root remains read-only until core exposes a dedicated setter.",
            self.settings_status_chip(status_label, status_accent),
        )
        .child(self.settings_readonly_path_card(
            "Database Root",
            state.storage.db_root.display().to_string(),
        ))
        .child(
            div()
                .h_flex()
                .flex_wrap()
                .gap(gpui::px(12.0))
                .child(self.settings_input_field(
                    "History Window (days)",
                    "Committed clipboard retention window.",
                    Input::new(&self.settings.history_window_input())
                        .small()
                        .appearance(false)
                        .bordered(false)
                        .focus_bordered(false)
                        .w_full(),
                ))
                .child(self.settings_input_field(
                    "Dedup Window (days)",
                    "Content dedup horizon for storage cleanup.",
                    Input::new(&self.settings.dedup_window_input())
                        .small()
                        .appearance(false)
                        .bordered(false)
                        .focus_bordered(false)
                        .w_full(),
                ))
                .child(self.settings_input_field(
                    "Max Text Bytes",
                    "Clipboard submit/edit ceiling enforced by core.",
                    Input::new(&self.settings.max_text_bytes_input())
                        .small()
                        .appearance(false)
                        .bordered(false)
                        .focus_bordered(false)
                        .w_full(),
                ))
                .child(self.settings_input_field(
                    "GC Batch Size",
                    "Per-pass cleanup batch size.",
                    Input::new(&self.settings.gc_batch_size_input())
                        .small()
                        .appearance(false)
                        .bordered(false)
                        .focus_bordered(false)
                        .w_full(),
                )),
        )
        .child(self.settings_storage_summary(&state.storage))
        .child(
            div().h_flex().justify_end().child(
                self.settings_action_button(
                    "settings-apply-storage",
                    "Apply Storage",
                    theme::accent_cyan(),
                    cx,
                )
                .disabled(!dirty || self.settings.applying().is_some())
                .on_click(cx.listener(|this, _, _, cx| {
                    this.request_apply_storage_settings(cx);
                })),
            ),
        )
    }
}
