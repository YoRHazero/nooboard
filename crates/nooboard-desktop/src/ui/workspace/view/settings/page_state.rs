use std::path::PathBuf;

use gpui::Context;
use gpui::{PathPromptOptions, Window};

use super::WorkspaceView;

#[derive(Clone, PartialEq, Eq)]
pub(in crate::ui::workspace::view) struct NetworkSettingsDraft {
    pub(super) network_enabled: bool,
    pub(super) mdns_enabled: bool,
}

#[derive(Clone, PartialEq, Eq)]
pub(in crate::ui::workspace::view) struct StorageSettingsDraft {
    pub(super) db_root: PathBuf,
    pub(super) retain_old_versions: usize,
    pub(super) history_window_days: u32,
    pub(super) dedup_window_days: u32,
    pub(super) gc_every_inserts: u32,
    pub(super) gc_batch_size: u32,
}

#[derive(Clone, PartialEq, Eq)]
pub(in crate::ui::workspace::view) struct SettingsDraft {
    pub(super) network: NetworkSettingsDraft,
    pub(super) storage: StorageSettingsDraft,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::ui::workspace::view) enum SettingsSaveState {
    Idle,
    Ready,
    Invalid,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::ui::workspace::view) enum StorageSettingField {
    RetainOldVersions,
    HistoryWindowDays,
    DedupWindowDays,
    GcEveryInserts,
    GcBatchSize,
}

impl StorageSettingField {
    pub(super) fn step(self) -> u32 {
        match self {
            Self::RetainOldVersions => 1,
            Self::HistoryWindowDays => 1,
            Self::DedupWindowDays => 1,
            Self::GcEveryInserts => 25,
            Self::GcBatchSize => 50,
        }
    }
}

pub(in crate::ui::workspace::view) struct SettingsPageState {
    pub(super) confirmed: SettingsDraft,
    pub(super) draft: SettingsDraft,
    pub(super) save_state: SettingsSaveState,
    pub(super) feedback: Option<String>,
}

impl SettingsPageState {
    pub(in crate::ui::workspace::view) fn new(network_enabled: bool) -> Self {
        let confirmed = SettingsDraft {
            network: NetworkSettingsDraft {
                network_enabled,
                mdns_enabled: true,
            },
            storage: StorageSettingsDraft {
                db_root: PathBuf::from(".dev-data"),
                retain_old_versions: 0,
                history_window_days: 7,
                dedup_window_days: 14,
                gc_every_inserts: 200,
                gc_batch_size: 500,
            },
        };

        Self {
            draft: confirmed.clone(),
            confirmed,
            save_state: SettingsSaveState::Idle,
            feedback: None,
        }
    }
}

impl WorkspaceView {
    pub(super) fn settings_feedback(&self) -> Option<&str> {
        self.settings_page_state.feedback.as_deref()
    }

    pub(super) fn settings_save_state(&self) -> SettingsSaveState {
        self.settings_page_state.save_state
    }

    pub(super) fn network_settings_draft(&self) -> &NetworkSettingsDraft {
        &self.settings_page_state.draft.network
    }

    pub(super) fn network_settings_confirmed(&self) -> &NetworkSettingsDraft {
        &self.settings_page_state.confirmed.network
    }

    pub(super) fn storage_settings_draft(&self) -> &StorageSettingsDraft {
        &self.settings_page_state.draft.storage
    }

    pub(super) fn storage_settings_confirmed(&self) -> &StorageSettingsDraft {
        &self.settings_page_state.confirmed.storage
    }

    pub(super) fn network_patch_fields(&self) -> Vec<&'static str> {
        let draft = self.network_settings_draft();
        let confirmed = self.network_settings_confirmed();
        let mut fields = Vec::new();

        if draft.network_enabled != confirmed.network_enabled {
            fields.push("network_enabled");
        }
        if draft.mdns_enabled != confirmed.mdns_enabled {
            fields.push("mdns_enabled");
        }

        fields
    }

    pub(super) fn network_patch_labels(&self) -> Vec<&'static str> {
        let draft = self.network_settings_draft();
        let confirmed = self.network_settings_confirmed();
        let mut fields = Vec::new();

        if draft.network_enabled != confirmed.network_enabled {
            fields.push("Network service");
        }
        if draft.mdns_enabled != confirmed.mdns_enabled {
            fields.push("Local discovery (mDNS)");
        }

        fields
    }

    pub(super) fn storage_patch_fields(&self) -> Vec<&'static str> {
        let draft = self.storage_settings_draft();
        let confirmed = self.storage_settings_confirmed();
        let mut fields = Vec::new();

        if draft.db_root != confirmed.db_root {
            fields.push("db_root");
        }
        if draft.retain_old_versions != confirmed.retain_old_versions {
            fields.push("retain_old_versions");
        }
        if draft.history_window_days != confirmed.history_window_days {
            fields.push("history_window_days");
        }
        if draft.dedup_window_days != confirmed.dedup_window_days {
            fields.push("dedup_window_days");
        }
        if draft.gc_every_inserts != confirmed.gc_every_inserts {
            fields.push("gc_every_inserts");
        }
        if draft.gc_batch_size != confirmed.gc_batch_size {
            fields.push("gc_batch_size");
        }

        fields
    }

    pub(super) fn storage_patch_labels(&self) -> Vec<&'static str> {
        let draft = self.storage_settings_draft();
        let confirmed = self.storage_settings_confirmed();
        let mut fields = Vec::new();

        if draft.db_root != confirmed.db_root {
            fields.push("Database root path");
        }
        if draft.retain_old_versions != confirmed.retain_old_versions {
            fields.push("Retained old versions");
        }
        if draft.history_window_days != confirmed.history_window_days {
            fields.push("History retention window");
        }
        if draft.dedup_window_days != confirmed.dedup_window_days {
            fields.push("Deduplication window");
        }
        if draft.gc_every_inserts != confirmed.gc_every_inserts {
            fields.push("Cleanup trigger interval");
        }
        if draft.gc_batch_size != confirmed.gc_batch_size {
            fields.push("Cleanup batch size");
        }

        fields
    }

    pub(super) fn settings_dirty_field_count(&self) -> usize {
        self.network_patch_fields().len() + self.storage_patch_fields().len()
    }

    pub(super) fn network_settings_dirty(&self) -> bool {
        !self.network_patch_fields().is_empty()
    }

    pub(super) fn storage_settings_dirty(&self) -> bool {
        !self.storage_patch_fields().is_empty()
    }

    pub(super) fn storage_validation_issues(&self) -> Vec<String> {
        let storage = self.storage_settings_draft();
        let mut issues = Vec::new();

        if storage.db_root.as_os_str().is_empty() {
            issues.push("Database root path cannot be empty".to_string());
        }
        if storage.history_window_days == 0 {
            issues.push("History retention window must be at least 1 day".to_string());
        }
        if storage.dedup_window_days == 0 {
            issues.push("Deduplication window must be at least 1 day".to_string());
        }
        if storage.gc_every_inserts == 0 {
            issues.push("Cleanup trigger interval must be at least 1".to_string());
        }
        if storage.gc_batch_size == 0 {
            issues.push("Cleanup batch size must be at least 1".to_string());
        }

        issues
    }

    fn set_settings_feedback(&mut self, save_state: SettingsSaveState, message: impl Into<String>) {
        self.settings_page_state.save_state = save_state;
        self.settings_page_state.feedback = Some(message.into());
    }

    fn clear_settings_feedback(&mut self) {
        self.settings_page_state.feedback = None;
        self.settings_page_state.save_state = if self.storage_validation_issues().is_empty() {
            SettingsSaveState::Idle
        } else {
            SettingsSaveState::Invalid
        };
    }

    pub(super) fn stage_storage_patch(&mut self, cx: &mut Context<Self>) {
        let fields = self.storage_patch_fields();
        let labels = self.storage_patch_labels();
        let issues = self.storage_validation_issues();

        if fields.is_empty() {
            self.set_settings_feedback(
                SettingsSaveState::Idle,
                "Storage draft matches the current settings.",
            );
        } else if !issues.is_empty() {
            self.set_settings_feedback(
                SettingsSaveState::Invalid,
                format!("Storage review is blocked: {}.", issues.join("; ")),
            );
        } else {
            self.set_settings_feedback(
                SettingsSaveState::Ready,
                format!("Storage changes ready for review: {}.", labels.join(", ")),
            );
        }
        cx.notify();
    }

    pub(super) fn stage_network_patch(&mut self, cx: &mut Context<Self>) {
        let fields = self.network_patch_fields();
        let labels = self.network_patch_labels();

        if fields.is_empty() {
            self.set_settings_feedback(
                SettingsSaveState::Idle,
                "Network draft matches the current settings.",
            );
        } else {
            self.set_settings_feedback(
                SettingsSaveState::Ready,
                format!("Network changes ready for review: {}.", labels.join(", ")),
            );
        }
        cx.notify();
    }

    pub(super) fn reset_storage_settings_draft(&mut self, cx: &mut Context<Self>) {
        self.settings_page_state.draft.storage = self.settings_page_state.confirmed.storage.clone();
        self.set_settings_feedback(
            SettingsSaveState::Idle,
            "Storage draft reset to the current settings.",
        );
        cx.notify();
    }

    pub(super) fn reset_network_settings_draft(&mut self, cx: &mut Context<Self>) {
        self.settings_page_state.draft.network = self.settings_page_state.confirmed.network.clone();
        self.set_settings_feedback(
            SettingsSaveState::Idle,
            "Network draft reset to the current settings.",
        );
        cx.notify();
    }

    pub(super) fn toggle_settings_network_enabled(&mut self, cx: &mut Context<Self>) {
        self.settings_page_state.draft.network.network_enabled =
            !self.settings_page_state.draft.network.network_enabled;
        self.clear_settings_feedback();
        cx.notify();
    }

    pub(super) fn toggle_settings_mdns_enabled(&mut self, cx: &mut Context<Self>) {
        self.settings_page_state.draft.network.mdns_enabled =
            !self.settings_page_state.draft.network.mdns_enabled;
        self.clear_settings_feedback();
        cx.notify();
    }

    pub(super) fn step_storage_setting(
        &mut self,
        field: StorageSettingField,
        increment: bool,
        cx: &mut Context<Self>,
    ) {
        let storage = &mut self.settings_page_state.draft.storage;

        match field {
            StorageSettingField::RetainOldVersions => {
                let step = field.step() as usize;
                storage.retain_old_versions = if increment {
                    storage.retain_old_versions.saturating_add(step)
                } else {
                    storage.retain_old_versions.saturating_sub(step)
                };
            }
            StorageSettingField::HistoryWindowDays => {
                let step = field.step();
                storage.history_window_days = if increment {
                    storage.history_window_days.saturating_add(step)
                } else {
                    storage.history_window_days.saturating_sub(step).max(1)
                };
            }
            StorageSettingField::DedupWindowDays => {
                let step = field.step();
                storage.dedup_window_days = if increment {
                    storage.dedup_window_days.saturating_add(step)
                } else {
                    storage.dedup_window_days.saturating_sub(step).max(1)
                };
            }
            StorageSettingField::GcEveryInserts => {
                let step = field.step();
                storage.gc_every_inserts = if increment {
                    storage.gc_every_inserts.saturating_add(step)
                } else {
                    storage.gc_every_inserts.saturating_sub(step).max(1)
                };
            }
            StorageSettingField::GcBatchSize => {
                let step = field.step();
                storage.gc_batch_size = if increment {
                    storage.gc_batch_size.saturating_add(step)
                } else {
                    storage.gc_batch_size.saturating_sub(step).max(1)
                };
            }
        }

        self.clear_settings_feedback();
        cx.notify();
    }

    pub(super) fn pick_settings_db_root(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let paths_receiver = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Select storage db root".into()),
        });
        let view = cx.entity().downgrade();

        cx.spawn_in(window, async move |_, cx| {
            let path = match paths_receiver.await {
                Ok(Ok(Some(mut paths))) => paths.drain(..).next(),
                _ => None,
            };

            let Some(path) = path else {
                return;
            };

            let _ = view.update(cx, |this, cx| {
                this.settings_page_state.draft.storage.db_root = path.clone();
                this.set_settings_feedback(
                    SettingsSaveState::Idle,
                    format!("Draft database root path updated to {}.", path.display()),
                );
                cx.notify();
            });
        })
        .detach();
    }
}
