use gpui::{
    App, Context, Div, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement,
    Styled, Window, div, px,
};
use gpui_component::Disableable;

use crate::ui::theme;

use super::StorageSettingField;
use super::WorkspaceView;
use super::components::{
    settings_action_button, settings_action_row, settings_control_button, settings_path_field_row,
    settings_section_footer, settings_section_shell, settings_status_chip,
    settings_stepper_field_row, settings_themed_tooltip,
};
use super::{SettingsSectionPhase, settings_status_tokens};

impl WorkspaceView {
    pub(super) fn storage_settings_panel(&self, cx: &mut Context<Self>) -> Div {
        let dirty = !self.storage_patch_labels().is_empty();
        let issues = self.storage_validation_issues();
        let patch_labels = self.storage_patch_labels();
        let (status_label, status_accent) = settings_status_tokens(self.storage_settings_status());
        let status = settings_status_chip(status_label, status_accent);

        settings_section_shell(
            "Storage",
            "Edit the app's persistent storage paths and retention windows.",
            status,
        )
        .child(self.settings_db_root_row(cx))
        .child(
            self.settings_number_row(
                "history_days",
                "History retention window",
                "How long clipboard history should stay available before it expires.",
                StorageSettingField::HistoryWindowDays,
                self.storage_settings_draft()
                    .history_window_days
                    .to_string(),
                self.storage_settings_confirmed()
                    .history_window_days
                    .to_string(),
                cx,
            ),
        )
        .child(
            self.settings_number_row(
                "dedup_days",
                "Deduplication window",
                "How far back the app should look when merging repeated text.",
                StorageSettingField::DedupWindowDays,
                self.storage_settings_draft().dedup_window_days.to_string(),
                self.storage_settings_confirmed()
                    .dedup_window_days
                    .to_string(),
                cx,
            ),
        )
        .child(self.settings_number_row(
            "max_text_bytes",
            "Maximum text bytes",
            "Upper bound for a single committed text record before the app rejects it.",
            StorageSettingField::MaxTextBytes,
            self.storage_settings_draft().max_text_bytes.to_string(),
            self.storage_settings_confirmed().max_text_bytes.to_string(),
            cx,
        ))
        .child(self.settings_number_row(
            "gc_batch_size",
            "Cleanup batch size",
            "How many records each cleanup pass should process at most.",
            StorageSettingField::GcBatchSize,
            self.storage_settings_draft().gc_batch_size.to_string(),
            self.storage_settings_confirmed().gc_batch_size.to_string(),
            cx,
        ))
        .child(settings_section_footer(
            if !issues.is_empty() {
                format!("Validation: {}", issues.join("; "))
            } else if patch_labels.is_empty() {
                "Storage settings match the current app state.".to_string()
            } else if let Some(message) = Self::settings_phase_summary(
                self.storage_settings_phase(),
                "Live storage settings changed while this draft was open.",
            ) {
                message
            } else {
                format!("Changed: {}", patch_labels.join(", "))
            },
            if issues.is_empty() {
                theme::fg_muted()
            } else {
                theme::accent_rose()
            },
            settings_action_row([
                settings_action_button(
                    "settings-reset-storage-draft",
                    "Reset",
                    theme::accent_rose(),
                    cx,
                )
                .disabled(!dirty)
                .on_click(cx.listener(|this, _, _, cx| {
                    this.reset_storage_settings_draft(cx);
                }))
                .into_any_element(),
                settings_action_button(
                    "settings-apply-storage-patch",
                    "Apply",
                    theme::accent_cyan(),
                    cx,
                )
                .disabled(
                    !dirty
                        || !issues.is_empty()
                        || matches!(
                            self.storage_settings_phase(),
                            SettingsSectionPhase::Applying
                        ),
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.apply_storage_settings(cx);
                }))
                .into_any_element(),
            ]),
        ))
    }

    fn settings_db_root_row(&self, cx: &mut Context<Self>) -> Div {
        let folder_label = self.storage_settings_draft().db_root.display().to_string();
        let confirmed_label = self
            .storage_settings_confirmed()
            .db_root
            .display()
            .to_string();
        let dirty =
            self.storage_settings_draft().db_root != self.storage_settings_confirmed().db_root;
        let tooltip = if dirty {
            format!(
                "Click to choose a different folder for the draft.\nCurrent path: {}",
                confirmed_label
            )
        } else {
            "Click to choose the folder used for the database root path.".to_string()
        };

        settings_path_field_row(
            "Database root path",
            "Click the path field to choose the folder used for storage.",
            confirmed_label,
            dirty,
            div()
                .id("settings-storage-db-root")
                .h(px(42.0))
                .w_full()
                .px(px(12.0))
                .py(px(10.0))
                .bg(theme::bg_console())
                .border_1()
                .border_color(if dirty {
                    theme::accent_amber().opacity(0.28)
                } else {
                    theme::border_soft()
                })
                .rounded(px(14.0))
                .cursor_pointer()
                .hover(|this| {
                    this.bg(theme::bg_panel_alt())
                        .border_color(theme::border_strong())
                })
                .active(|this| this.bg(theme::bg_panel()))
                .tooltip(move |window: &mut Window, cx: &mut App| {
                    settings_themed_tooltip(tooltip.clone(), window, cx)
                })
                .on_click(cx.listener(|this, _, window, cx| {
                    this.pick_settings_db_root(window, cx);
                }))
                .child(
                    div()
                        .w_full()
                        .min_w(px(0.0))
                        .text_size(px(12.0))
                        .text_color(theme::fg_secondary())
                        .line_clamp(1)
                        .text_ellipsis()
                        .child(folder_label),
                ),
        )
    }

    fn settings_number_row(
        &self,
        id_suffix: &'static str,
        label: &'static str,
        hint: &'static str,
        field: StorageSettingField,
        value: String,
        confirmed: String,
        cx: &mut Context<Self>,
    ) -> Div {
        let dirty = value != confirmed;
        let decrement_id = format!("settings-step-down-{id_suffix}");
        let increment_id = format!("settings-step-up-{id_suffix}");

        settings_stepper_field_row(
            label,
            hint,
            value,
            confirmed,
            field.step(),
            dirty,
            settings_control_button(decrement_id, "-", theme::accent_rose(), cx).on_click(
                cx.listener(move |this, _, _, cx| {
                    this.step_storage_setting(field, false, cx);
                }),
            ),
            settings_control_button(increment_id, "+", theme::accent_green(), cx).on_click(
                cx.listener(move |this, _, _, cx| {
                    this.step_storage_setting(field, true, cx);
                }),
            ),
        )
    }
}
