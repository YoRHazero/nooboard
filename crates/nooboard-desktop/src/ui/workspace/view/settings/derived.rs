use gpui::Context;

use super::patches::{
    clipboard_patch_labels, network_patch_labels, network_validation_issues,
    normalized_listen_port, storage_patch_labels, storage_validation_issues, transfer_patch_labels,
    transfer_validation_issues,
};
use super::snapshot::{
    ClipboardSettingsValue, LocalConnectionInfoValue, NetworkPanelValue, StorageSettingsValue,
    TransferSettingsValue,
};
use super::{
    SettingsSectionPhase, SettingsStatus, WorkspaceView, build_settings_snapshot,
    settings_section_status,
};

impl WorkspaceView {
    pub(in crate::ui::workspace::view) fn sync_settings_page_state(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        let snapshot = {
            let store = self.live_store.read(cx);
            build_settings_snapshot(&store)
        };

        self.settings_page_state.sync_from_snapshot(snapshot);
    }

    pub(super) fn settings_feedback(&self) -> Option<&str> {
        self.settings_page_state.feedback.as_deref()
    }

    pub(super) fn settings_dirty_field_count(&self) -> usize {
        self.network_patch_labels().len()
            + self.storage_patch_labels().len()
            + self.clipboard_patch_labels().len()
            + self.transfer_patch_labels().len()
    }

    pub(super) fn settings_status(&self) -> SettingsStatus {
        let has_applying = matches!(
            self.settings_page_state.network.phase,
            SettingsSectionPhase::Applying
        ) || matches!(
            self.settings_page_state.storage.phase,
            SettingsSectionPhase::Applying
        ) || matches!(
            self.settings_page_state.clipboard.phase,
            SettingsSectionPhase::Applying
        ) || matches!(
            self.settings_page_state.transfers.phase,
            SettingsSectionPhase::Applying
        );
        let has_errors = matches!(
            self.settings_page_state.network.phase,
            SettingsSectionPhase::Error(_)
        ) || matches!(
            self.settings_page_state.storage.phase,
            SettingsSectionPhase::Error(_)
        ) || matches!(
            self.settings_page_state.clipboard.phase,
            SettingsSectionPhase::Error(_)
        ) || matches!(
            self.settings_page_state.transfers.phase,
            SettingsSectionPhase::Error(_)
        );
        let has_stale = matches!(
            self.settings_page_state.network.phase,
            SettingsSectionPhase::Stale
        ) || matches!(
            self.settings_page_state.storage.phase,
            SettingsSectionPhase::Stale
        ) || matches!(
            self.settings_page_state.clipboard.phase,
            SettingsSectionPhase::Stale
        ) || matches!(
            self.settings_page_state.transfers.phase,
            SettingsSectionPhase::Stale
        );
        let has_validation_issues = !self.network_validation_issues().is_empty()
            || !self.storage_validation_issues().is_empty()
            || !self.transfer_validation_issues().is_empty();

        if has_applying {
            SettingsStatus::Applying
        } else if has_validation_issues {
            SettingsStatus::Review
        } else if has_errors {
            SettingsStatus::Error
        } else if has_stale {
            SettingsStatus::Stale
        } else if self.settings_dirty_field_count() > 0 {
            SettingsStatus::Modified
        } else {
            SettingsStatus::Current
        }
    }

    pub(super) fn settings_status_message(&self) -> String {
        if let Some(feedback) = self.settings_feedback() {
            return feedback.to_string();
        }

        match self.settings_status() {
            SettingsStatus::Applying => {
                "Waiting for app state to confirm the applied settings.".to_string()
            }
            SettingsStatus::Review => {
                let mut issues = self.network_validation_issues();
                issues.extend(self.storage_validation_issues());
                issues.extend(self.transfer_validation_issues());
                issues.join("; ")
            }
            SettingsStatus::Error => self
                .settings_section_error()
                .unwrap_or("Applying settings failed.".to_string()),
            SettingsStatus::Stale => {
                "The live app settings changed while this draft was open.".to_string()
            }
            SettingsStatus::Modified => {
                "Local draft values differ from the current app settings.".to_string()
            }
            SettingsStatus::Current => "Settings match the current app state.".to_string(),
        }
    }

    pub(super) fn network_settings_draft(&self) -> &NetworkPanelValue {
        &self.settings_page_state.network.draft
    }

    pub(super) fn network_settings_confirmed(&self) -> &NetworkPanelValue {
        &self.settings_page_state.network.baseline
    }

    pub(super) fn local_connection_info(&self) -> &LocalConnectionInfoValue {
        &self.settings_page_state.local_connection
    }

    pub(super) fn network_device_ip_label(&self) -> String {
        self.local_connection_info()
            .device_endpoint
            .map(|endpoint| endpoint.ip().to_string())
            .unwrap_or_else(|| "Unavailable".to_string())
    }

    pub(super) fn network_device_endpoint_preview(&self) -> Option<String> {
        let ip = self.local_connection_info().device_endpoint?.ip();
        let port = normalized_listen_port(&self.network_settings_draft().listen_port_text)?;

        Some(format!("{ip}:{port}"))
    }

    pub(super) fn network_device_endpoint_display(&self) -> String {
        self.network_device_endpoint_preview()
            .unwrap_or_else(|| "Unavailable".to_string())
    }

    pub(super) fn storage_settings_draft(&self) -> &StorageSettingsValue {
        &self.settings_page_state.storage.draft
    }

    pub(super) fn storage_settings_confirmed(&self) -> &StorageSettingsValue {
        &self.settings_page_state.storage.baseline
    }

    pub(super) fn clipboard_settings_draft(&self) -> &ClipboardSettingsValue {
        &self.settings_page_state.clipboard.draft
    }

    pub(super) fn clipboard_settings_confirmed(&self) -> &ClipboardSettingsValue {
        &self.settings_page_state.clipboard.baseline
    }

    pub(super) fn transfer_settings_draft(&self) -> &TransferSettingsValue {
        &self.settings_page_state.transfers.draft
    }

    pub(super) fn transfer_settings_confirmed(&self) -> &TransferSettingsValue {
        &self.settings_page_state.transfers.baseline
    }

    pub(super) fn network_patch_labels(&self) -> Vec<&'static str> {
        network_patch_labels(
            self.network_settings_confirmed(),
            self.network_settings_draft(),
        )
    }

    pub(super) fn storage_patch_labels(&self) -> Vec<&'static str> {
        storage_patch_labels(
            self.storage_settings_confirmed(),
            self.storage_settings_draft(),
        )
    }

    pub(super) fn clipboard_patch_labels(&self) -> Vec<&'static str> {
        clipboard_patch_labels(
            self.clipboard_settings_confirmed(),
            self.clipboard_settings_draft(),
        )
    }

    pub(super) fn transfer_patch_labels(&self) -> Vec<&'static str> {
        transfer_patch_labels(
            self.transfer_settings_confirmed(),
            self.transfer_settings_draft(),
        )
    }

    pub(super) fn network_settings_status(&self) -> SettingsStatus {
        settings_section_status(
            &self.settings_page_state.network,
            !self.network_validation_issues().is_empty(),
        )
    }

    pub(super) fn storage_settings_status(&self) -> SettingsStatus {
        settings_section_status(
            &self.settings_page_state.storage,
            !self.storage_validation_issues().is_empty(),
        )
    }

    pub(super) fn clipboard_settings_status(&self) -> SettingsStatus {
        settings_section_status(&self.settings_page_state.clipboard, false)
    }

    pub(super) fn transfer_settings_status(&self) -> SettingsStatus {
        settings_section_status(
            &self.settings_page_state.transfers,
            !self.transfer_validation_issues().is_empty(),
        )
    }

    pub(super) fn network_validation_issues(&self) -> Vec<String> {
        network_validation_issues(self.network_settings_draft())
    }

    pub(super) fn storage_validation_issues(&self) -> Vec<String> {
        storage_validation_issues(self.storage_settings_draft())
    }

    pub(super) fn transfer_validation_issues(&self) -> Vec<String> {
        transfer_validation_issues(self.transfer_settings_draft())
    }

    pub(super) fn network_settings_phase(&self) -> &SettingsSectionPhase {
        &self.settings_page_state.network.phase
    }

    pub(super) fn storage_settings_phase(&self) -> &SettingsSectionPhase {
        &self.settings_page_state.storage.phase
    }

    pub(super) fn clipboard_settings_phase(&self) -> &SettingsSectionPhase {
        &self.settings_page_state.clipboard.phase
    }

    pub(super) fn transfer_settings_phase(&self) -> &SettingsSectionPhase {
        &self.settings_page_state.transfers.phase
    }

    pub(in crate::ui::workspace::view::settings) fn set_settings_feedback(
        &mut self,
        message: impl Into<String>,
    ) {
        self.settings_page_state.feedback = Some(message.into());
    }

    pub(in crate::ui::workspace::view::settings) fn clear_settings_feedback(&mut self) {
        self.settings_page_state.feedback = None;
    }

    fn settings_section_error(&self) -> Option<String> {
        if let SettingsSectionPhase::Error(message) = &self.settings_page_state.network.phase {
            Some(message.clone())
        } else if let SettingsSectionPhase::Error(message) = &self.settings_page_state.storage.phase
        {
            Some(message.clone())
        } else if let SettingsSectionPhase::Error(message) =
            &self.settings_page_state.clipboard.phase
        {
            Some(message.clone())
        } else if let SettingsSectionPhase::Error(message) =
            &self.settings_page_state.transfers.phase
        {
            Some(message.clone())
        } else {
            None
        }
    }
}
