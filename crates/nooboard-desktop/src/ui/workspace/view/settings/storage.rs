use gpui::{
    App, Context, Div, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use gpui_component::Disableable;

use crate::ui::theme;

use super::components::{
    settings_action_button, settings_action_row, settings_button_with_tooltip,
    settings_control_button, settings_section_footer, settings_section_shell, settings_status_chip,
    settings_themed_tooltip, settings_path_field_row, settings_stepper_field_row,
};
use super::WorkspaceView;
use super::StorageSettingField;

impl WorkspaceView {
    pub(super) fn storage_settings_panel(&self, cx: &mut Context<Self>) -> Div {
        let dirty = self.storage_settings_dirty();
        let issues = self.storage_validation_issues();
        let patch_labels = self.storage_patch_labels();
        let status = if !issues.is_empty() {
            settings_status_chip("Review", theme::accent_rose())
        } else if dirty {
            settings_status_chip("Modified", theme::accent_amber())
        } else {
            settings_status_chip("Current", theme::accent_green())
        };

        settings_section_shell(
            "Storage",
            "Adjust the draft settings for retention, cleanup, and storage location.",
            status,
        )
            .child(self.settings_db_root_row(cx))
            .child(self.settings_number_row(
                "retain_versions",
                "Retained old versions",
                "How many older revisions should stay available after cleanup runs.",
                StorageSettingField::RetainOldVersions,
                self.storage_settings_draft().retain_old_versions.to_string(),
                self.storage_settings_confirmed()
                    .retain_old_versions
                    .to_string(),
                cx,
            ))
            .child(self.settings_number_row(
                "history_days",
                "History retention window",
                "How long clipboard history should stay available before it expires.",
                StorageSettingField::HistoryWindowDays,
                self.storage_settings_draft().history_window_days.to_string(),
                self.storage_settings_confirmed()
                    .history_window_days
                    .to_string(),
                cx,
            ))
            .child(self.settings_number_row(
                "dedup_days",
                "Deduplication window",
                "How far back the app should look when merging repeated text.",
                StorageSettingField::DedupWindowDays,
                self.storage_settings_draft().dedup_window_days.to_string(),
                self.storage_settings_confirmed().dedup_window_days.to_string(),
                cx,
            ))
            .child(self.settings_number_row(
                "gc_every_inserts",
                "Cleanup trigger interval",
                "How many inserts should happen before a cleanup pass starts.",
                StorageSettingField::GcEveryInserts,
                self.storage_settings_draft().gc_every_inserts.to_string(),
                self.storage_settings_confirmed().gc_every_inserts.to_string(),
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
                    "No storage changes ready for review.".to_string()
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
                    settings_button_with_tooltip(
                        "settings-review-storage-tooltip",
                        settings_action_button(
                            "settings-stage-storage-patch",
                            "Review",
                            theme::accent_cyan(),
                            cx,
                        )
                        .disabled(!dirty || !issues.is_empty())
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.stage_storage_patch(cx);
                        })),
                        Some(
                            "Preview which storage settings would be prepared from this draft. No backend call happens yet.",
                        ),
                    ),
                ]),
            ))
    }

    fn settings_db_root_row(&self, cx: &mut Context<Self>) -> Div {
        let folder_label = self
            .storage_settings_draft()
            .db_root
            .display()
            .to_string();
        let confirmed_label = self
            .storage_settings_confirmed()
            .db_root
            .display()
            .to_string();
        let dirty = self.storage_settings_draft().db_root != self.storage_settings_confirmed().db_root;
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
            settings_control_button(
                decrement_id,
                "-",
                theme::accent_rose(),
                cx,
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                this.step_storage_setting(field, false, cx);
            })),
            settings_control_button(
                increment_id,
                "+",
                theme::accent_green(),
                cx,
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                this.step_storage_setting(field, true, cx);
            })),
        )
    }
}
