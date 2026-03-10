use anyhow::Error;
use gpui::Context;

use crate::state::live_commands;

use super::WorkspaceView;
use super::patches::{
    build_clipboard_patch, build_network_patches, build_storage_patch, build_transfer_patch,
};

impl WorkspaceView {
    pub(super) fn apply_network_settings(&mut self, cx: &mut Context<Self>) {
        let issues = self.network_validation_issues();
        if !issues.is_empty() {
            self.set_settings_feedback(format!(
                "Network settings need review: {}.",
                issues.join("; ")
            ));
            cx.notify();
            return;
        }

        let patches = build_network_patches(
            self.network_settings_confirmed(),
            self.network_settings_draft(),
        );
        if patches.is_empty() {
            self.set_settings_feedback("Network settings already match the app state.");
            cx.notify();
            return;
        }

        let commands = live_commands::client(cx);
        let view = cx.entity().downgrade();
        self.settings_page_state.network.begin_apply();
        self.set_settings_feedback("Applying network settings.");
        cx.notify();

        cx.spawn(async move |_, cx| {
            let result = async {
                for patch in patches {
                    commands.patch_settings(patch, cx).await?;
                }
                Ok::<_, nooboard_app::AppError>(())
            }
            .await;

            let _ = view.update(cx, |this, cx| {
                match result {
                    Ok(()) => {
                        this.set_settings_feedback(
                            "Network settings submitted. Waiting for app state confirmation.",
                        );
                    }
                    Err(error) => {
                        this.settings_page_state
                            .network
                            .mark_error(format!("Failed to apply network settings: {error}"));
                        this.set_settings_feedback(format!(
                            "Failed to apply network settings: {error}"
                        ));
                    }
                }
                cx.notify();
            });
            Ok::<_, Error>(())
        })
        .detach();
    }

    pub(super) fn apply_storage_settings(&mut self, cx: &mut Context<Self>) {
        let issues = self.storage_validation_issues();
        if !issues.is_empty() {
            self.set_settings_feedback(format!(
                "Storage settings need review: {}.",
                issues.join("; ")
            ));
            cx.notify();
            return;
        }

        let Some(patch) = build_storage_patch(
            self.storage_settings_confirmed(),
            self.storage_settings_draft(),
        ) else {
            self.set_settings_feedback("Storage settings already match the app state.");
            cx.notify();
            return;
        };

        let commands = live_commands::client(cx);
        let view = cx.entity().downgrade();
        self.settings_page_state.storage.begin_apply();
        self.set_settings_feedback("Applying storage settings.");
        cx.notify();

        cx.spawn(async move |_, cx| {
            let result = commands.patch_settings(patch, cx).await;

            let _ = view.update(cx, |this, cx| {
                match result {
                    Ok(()) => {
                        this.set_settings_feedback(
                            "Storage settings submitted. Waiting for app state confirmation.",
                        );
                    }
                    Err(error) => {
                        this.settings_page_state
                            .storage
                            .mark_error(format!("Failed to apply storage settings: {error}"));
                        this.set_settings_feedback(format!(
                            "Failed to apply storage settings: {error}"
                        ));
                    }
                }
                cx.notify();
            });
            Ok::<_, Error>(())
        })
        .detach();
    }

    pub(super) fn apply_clipboard_settings(&mut self, cx: &mut Context<Self>) {
        let Some(patch) = build_clipboard_patch(
            self.clipboard_settings_confirmed(),
            self.clipboard_settings_draft(),
        ) else {
            self.set_settings_feedback("Clipboard settings already match the app state.");
            cx.notify();
            return;
        };

        let commands = live_commands::client(cx);
        let view = cx.entity().downgrade();
        self.settings_page_state.clipboard.begin_apply();
        self.set_settings_feedback("Applying clipboard settings.");
        cx.notify();

        cx.spawn(async move |_, cx| {
            let result = commands.patch_settings(patch, cx).await;

            let _ = view.update(cx, |this, cx| {
                match result {
                    Ok(()) => {
                        this.set_settings_feedback(
                            "Clipboard settings submitted. Waiting for app state confirmation.",
                        );
                    }
                    Err(error) => {
                        this.settings_page_state
                            .clipboard
                            .mark_error(format!("Failed to apply clipboard settings: {error}"));
                        this.set_settings_feedback(format!(
                            "Failed to apply clipboard settings: {error}"
                        ));
                    }
                }
                cx.notify();
            });
            Ok::<_, Error>(())
        })
        .detach();
    }

    pub(super) fn apply_transfer_settings(&mut self, cx: &mut Context<Self>) {
        let issues = self.transfer_validation_issues();
        if !issues.is_empty() {
            self.set_settings_feedback(format!(
                "Transfer settings need review: {}.",
                issues.join("; ")
            ));
            cx.notify();
            return;
        }

        let Some(patch) = build_transfer_patch(
            self.transfer_settings_confirmed(),
            self.transfer_settings_draft(),
        ) else {
            self.set_settings_feedback("Transfer settings already match the app state.");
            cx.notify();
            return;
        };

        let commands = live_commands::client(cx);
        let view = cx.entity().downgrade();
        self.settings_page_state.transfers.begin_apply();
        self.set_settings_feedback("Applying transfer settings.");
        cx.notify();

        cx.spawn(async move |_, cx| {
            let result = commands.patch_settings(patch, cx).await;

            let _ = view.update(cx, |this, cx| {
                match result {
                    Ok(()) => {
                        this.set_settings_feedback(
                            "Transfer settings submitted. Waiting for app state confirmation.",
                        );
                    }
                    Err(error) => {
                        this.settings_page_state
                            .transfers
                            .mark_error(format!("Failed to apply transfer settings: {error}"));
                        this.set_settings_feedback(format!(
                            "Failed to apply transfer settings: {error}"
                        ));
                    }
                }
                cx.notify();
            });
            Ok::<_, Error>(())
        })
        .detach();
    }
}
