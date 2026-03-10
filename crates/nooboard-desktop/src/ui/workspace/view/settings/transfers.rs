use gpui::{
    App, Context, Div, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement,
    Styled, Window, div, px,
};
use gpui_component::Disableable;

use crate::ui::theme;

use super::WorkspaceView;
use super::components::{
    settings_action_button, settings_action_row, settings_path_field_row, settings_section_footer,
    settings_section_shell, settings_status_chip, settings_themed_tooltip,
};
use super::{SettingsSectionPhase, settings_status_tokens};

impl WorkspaceView {
    pub(super) fn transfer_settings_panel(&self, cx: &mut Context<Self>) -> Div {
        let dirty = !self.transfer_patch_labels().is_empty();
        let issues = self.transfer_validation_issues();
        let (status_label, status_accent) = settings_status_tokens(self.transfer_settings_status());
        let status = settings_status_chip(status_label, status_accent);

        settings_section_shell(
            "Transfers",
            "Edit the download directory used by incoming file transfers.",
            status,
        )
        .child(self.settings_download_dir_row(cx))
        .child(settings_section_footer(
            if !issues.is_empty() {
                format!("Validation: {}", issues.join("; "))
            } else if self.transfer_patch_labels().is_empty() {
                "Transfer settings match the current app state.".to_string()
            } else if let Some(message) = Self::settings_phase_summary(
                self.transfer_settings_phase(),
                "Live transfer settings changed while this draft was open.",
            ) {
                message
            } else {
                format!("Changed: {}", self.transfer_patch_labels().join(", "))
            },
            if issues.is_empty() {
                theme::fg_muted()
            } else {
                theme::accent_rose()
            },
            settings_action_row([
                settings_action_button(
                    "settings-reset-transfer-draft",
                    "Reset",
                    theme::accent_rose(),
                    cx,
                )
                .disabled(!dirty)
                .on_click(cx.listener(|this, _, _, cx| {
                    this.reset_transfer_settings_draft(cx);
                }))
                .into_any_element(),
                settings_action_button(
                    "settings-apply-transfer-patch",
                    "Apply",
                    theme::accent_cyan(),
                    cx,
                )
                .disabled(
                    !dirty
                        || !issues.is_empty()
                        || matches!(
                            self.transfer_settings_phase(),
                            SettingsSectionPhase::Applying
                        ),
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.apply_transfer_settings(cx);
                }))
                .into_any_element(),
            ]),
        ))
    }

    fn settings_download_dir_row(&self, cx: &mut Context<Self>) -> Div {
        let folder_label = self
            .transfer_settings_draft()
            .download_dir
            .display()
            .to_string();
        let confirmed_label = self
            .transfer_settings_confirmed()
            .download_dir
            .display()
            .to_string();
        let dirty = self.transfer_settings_draft().download_dir
            != self.transfer_settings_confirmed().download_dir;
        let tooltip = if dirty {
            format!(
                "Click to choose a different download directory for the draft.\nCurrent path: {}",
                confirmed_label
            )
        } else {
            "Click to choose the directory used for incoming transfers.".to_string()
        };

        settings_path_field_row(
            "Download directory",
            "Transfers page reads this value directly and routes here when you want to change it.",
            confirmed_label,
            dirty,
            div()
                .id("settings-transfer-download-dir")
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
                    this.pick_settings_download_dir(window, cx);
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
}
