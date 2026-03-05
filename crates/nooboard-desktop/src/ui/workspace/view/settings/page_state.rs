use std::path::PathBuf;

use gpui::Context;
use gpui::{PathPromptOptions, Window};

use super::WorkspaceView;

pub(in crate::ui::workspace::view) struct SettingsPageState {
    pub(super) storage_db_root: PathBuf,
    pub(super) storage_retain_versions: String,
    pub(super) storage_history_days: String,
    pub(super) storage_dedup_days: String,
    pub(super) storage_gc_every_inserts: String,
    pub(super) storage_gc_batch_size: String,
    pub(super) network_enabled: bool,
    pub(super) mdns_enabled: bool,
    pub(super) feedback: Option<String>,
}

impl SettingsPageState {
    pub(in crate::ui::workspace::view) fn new(network_enabled: bool) -> Self {
        Self {
            storage_db_root: PathBuf::from(".dev-data/db"),
            storage_retain_versions: "0".to_string(),
            storage_history_days: "7".to_string(),
            storage_dedup_days: "14".to_string(),
            storage_gc_every_inserts: "5".to_string(),
            storage_gc_batch_size: "20".to_string(),
            network_enabled,
            mdns_enabled: true,
            feedback: None,
        }
    }
}

impl WorkspaceView {
    pub(super) fn settings_feedback(&self) -> Option<&str> {
        self.settings_page_state.feedback.as_deref()
    }

    pub(super) fn save_storage_patch(&mut self, cx: &mut Context<Self>) {
        self.settings_page_state.feedback = Some(format!(
            "Storage patch staged: db_root={}, retain_versions={}, history_days={}, dedup_days={}, gc_every_inserts={}, gc_batch_size={}",
            self.settings_page_state.storage_db_root.display(),
            self.settings_page_state.storage_retain_versions,
            self.settings_page_state.storage_history_days,
            self.settings_page_state.storage_dedup_days,
            self.settings_page_state.storage_gc_every_inserts,
            self.settings_page_state.storage_gc_batch_size,
        ));
        cx.notify();
    }

    pub(super) fn save_network_patch(&mut self, cx: &mut Context<Self>) {
        self.settings_page_state.feedback = Some(format!(
            "Network patch staged: network_enabled={}, mdns_enabled={}",
            if self.settings_page_state.network_enabled {
                "on"
            } else {
                "off"
            },
            if self.settings_page_state.mdns_enabled {
                "on"
            } else {
                "off"
            }
        ));
        cx.notify();
    }

    pub(super) fn toggle_settings_network_enabled(&mut self, cx: &mut Context<Self>) {
        self.settings_page_state.network_enabled = !self.settings_page_state.network_enabled;
        self.settings_page_state.feedback = Some(format!(
            "Network enabled set to {}.",
            if self.settings_page_state.network_enabled {
                "on"
            } else {
                "off"
            }
        ));
        cx.notify();
    }

    pub(super) fn toggle_settings_mdns_enabled(&mut self, cx: &mut Context<Self>) {
        self.settings_page_state.mdns_enabled = !self.settings_page_state.mdns_enabled;
        self.settings_page_state.feedback = Some(format!(
            "mDNS enabled set to {}.",
            if self.settings_page_state.mdns_enabled {
                "on"
            } else {
                "off"
            }
        ));
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
                this.settings_page_state.storage_db_root = path.clone();
                this.settings_page_state.feedback =
                    Some(format!("db_root set to {}", path.display()));
                cx.notify();
            });
        })
        .detach();
    }
}
