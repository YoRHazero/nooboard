use std::path::PathBuf;

use gpui::{AppContext as _, ClipboardItem, Context, PathPromptOptions, Window};
use nooboard_core::StorageSettingsInput;

use crate::{
    ui::workspace::WorkspaceView,
    workspace::{
        actions::settings::{
            self as settings_actions, ApplyClipboardSettingsInput, ApplyConnectionSettingsInput,
        },
        view_state::SettingsPageViewState,
    },
};

use super::state::{SettingsSectionKey, StorageBytesUnit, StorageDurationUnit};

impl WorkspaceView {
    pub(super) fn copy_settings_config_path(&mut self, path: String, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(path));
        self.settings
            .set_feedback("Config file path copied.".to_string());
        cx.notify();
    }

    pub(super) fn request_settings_toggle_lan_enabled(&mut self, cx: &mut Context<Self>) {
        self.settings.toggle_lan_enabled();
        cx.notify();
    }

    pub(super) fn request_settings_toggle_connection_token_mask(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.settings.toggle_token_masked(window, cx);
        cx.notify();
    }

    pub(super) fn request_settings_toggle_local_capture(&mut self, cx: &mut Context<Self>) {
        self.settings.toggle_local_capture_enabled();
        cx.notify();
    }

    pub(super) fn request_reset_clipboard_settings(&mut self, cx: &mut Context<Self>) {
        let Some(page) = self.current_settings_page(cx) else {
            self.settings
                .fail_apply("Settings are still loading.".to_string());
            cx.notify();
            return;
        };

        self.settings.reset_clipboard_from_workspace(&page.clipboard);
        self.settings
            .set_feedback("Clipboard settings restored.".to_string());
        cx.notify();
    }

    pub(super) fn request_reset_connection_settings(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(page) = self.current_settings_page(cx) else {
            self.settings
                .fail_apply("Settings are still loading.".to_string());
            cx.notify();
            return;
        };

        self.settings
            .reset_connection_from_workspace(&page.connection, window, cx);
        self.settings
            .set_feedback("Connection settings restored.".to_string());
        cx.notify();
    }

    pub(super) fn request_reset_transfer_settings(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(page) = self.current_settings_page(cx) else {
            self.settings
                .fail_apply("Settings are still loading.".to_string());
            cx.notify();
            return;
        };

        self.settings
            .reset_transfer_from_workspace(&page.transfers, window, cx);
        self.settings
            .set_feedback("Transfer settings restored.".to_string());
        cx.notify();
    }

    pub(super) fn request_reset_storage_settings(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(page) = self.current_settings_page(cx) else {
            self.settings
                .fail_apply("Settings are still loading.".to_string());
            cx.notify();
            return;
        };

        self.settings
            .reset_storage_from_workspace(&page.storage, window, cx);
        self.settings
            .set_feedback("Storage settings restored.".to_string());
        cx.notify();
    }

    pub(super) fn request_select_history_window_unit(
        &mut self,
        unit: StorageDurationUnit,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.settings.select_history_window_unit(unit, window, cx);
        cx.notify();
    }

    pub(super) fn request_select_dedup_window_unit(
        &mut self,
        unit: StorageDurationUnit,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.settings.select_dedup_window_unit(unit, window, cx);
        cx.notify();
    }

    pub(super) fn request_select_max_text_bytes_unit(
        &mut self,
        unit: StorageBytesUnit,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.settings.select_max_text_bytes_unit(unit, window, cx);
        cx.notify();
    }

    pub(super) fn pick_settings_download_dir(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let paths_receiver = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Select transfer download directory".into()),
        });
        let view = cx.entity().downgrade();
        let window_handle = window.window_handle();

        cx.spawn_in(window, async move |_, cx| {
            let path = match paths_receiver.await {
                Ok(Ok(Some(mut paths))) => paths.drain(..).next(),
                _ => None,
            };

            let Some(path) = path else {
                return;
            };

            let _ = cx.update_window(window_handle, |_, window, cx| {
                if let Some(view) = view.upgrade() {
                    let _ = view.update(cx, |this, cx| {
                        this.settings.set_download_dir_value(
                            path.display().to_string(),
                            window,
                            cx,
                        );
                        cx.notify();
                    });
                }
            });
        })
        .detach();
    }

    pub(super) fn request_apply_connection_settings(&mut self, cx: &mut Context<Self>) {
        let Some(page) = self.current_settings_page(cx) else {
            self.settings
                .fail_apply("Settings are still loading.".to_string());
            cx.notify();
            return;
        };
        let Ok(listen_port) = self.settings.listen_port_value(cx).trim().parse::<u16>() else {
            self.settings
                .fail_apply("Port must be a valid number.".to_string());
            cx.notify();
            return;
        };

        let Some(task) = settings_actions::apply_connection_settings_task(
            &self.controller,
            ApplyConnectionSettingsInput {
                device_id: self.settings.device_id_value(cx).trim().to_string(),
                token: self.settings.token_value(cx).trim().to_string(),
                listen_port,
                lan_enabled: self.settings.lan_enabled(),
                current_device_id: page.connection.device_id,
                current_token: page.connection.token,
                current_listen_port: page.connection.listen_port,
                current_lan_enabled: page.connection.lan_enabled,
            },
            cx,
        ) else {
            self.settings
                .fail_apply("Settings are still loading.".to_string());
            cx.notify();
            return;
        };

        self.settings.begin_apply(
            SettingsSectionKey::Connection,
            "Saving connection settings.".to_string(),
        );
        cx.notify();

        let view = cx.entity().downgrade();
        cx.spawn(async move |_, cx| {
            let result = task.await;
            let _ = view.update(cx, |this, cx| {
                match result {
                    Ok(()) => this
                        .settings
                        .finish_apply("Connection settings saved.".to_string()),
                    Err(error) => this
                        .settings
                        .fail_apply(format!("Couldn't save connection settings: {error}")),
                }
                cx.notify();
            });
            Ok::<_, anyhow::Error>(())
        })
        .detach();
    }

    pub(super) fn request_apply_clipboard_settings(&mut self, cx: &mut Context<Self>) {
        let Some(page) = self.current_settings_page(cx) else {
            self.settings
                .fail_apply("Settings are still loading.".to_string());
            cx.notify();
            return;
        };

        let Some(task) = settings_actions::apply_clipboard_settings_task(
            &self.controller,
            ApplyClipboardSettingsInput {
                local_capture_enabled: self.settings.local_capture_enabled(),
                current_local_capture_enabled: page.clipboard.local_capture_enabled,
            },
            cx,
        ) else {
            self.settings
                .fail_apply("Settings are still loading.".to_string());
            cx.notify();
            return;
        };

        self.settings.begin_apply(
            SettingsSectionKey::Clipboard,
            "Saving clipboard settings.".to_string(),
        );
        cx.notify();

        let view = cx.entity().downgrade();
        cx.spawn(async move |_, cx| {
            let result = task.await;
            let _ = view.update(cx, |this, cx| {
                match result {
                    Ok(()) => this
                        .settings
                        .finish_apply("Clipboard settings saved.".to_string()),
                    Err(error) => this
                        .settings
                        .fail_apply(format!("Couldn't save clipboard settings: {error}")),
                }
                cx.notify();
            });
            Ok::<_, anyhow::Error>(())
        })
        .detach();
    }

    pub(super) fn request_apply_transfer_settings(&mut self, cx: &mut Context<Self>) {
        let download_dir = self.settings.download_dir_value(cx).trim().to_string();
        if download_dir.is_empty() {
            self.settings
                .fail_apply("Download directory cannot be empty.".to_string());
            cx.notify();
            return;
        }

        let Some(task) = settings_actions::set_download_dir_task(
            &self.controller,
            PathBuf::from(download_dir),
            cx,
        ) else {
            self.settings
                .fail_apply("Settings are still loading.".to_string());
            cx.notify();
            return;
        };

        self.settings.begin_apply(
            SettingsSectionKey::Transfers,
            "Saving transfer settings.".to_string(),
        );
        cx.notify();

        let view = cx.entity().downgrade();
        cx.spawn(async move |_, cx| {
            let result = task.await;
            let _ = view.update(cx, |this, cx| {
                match result {
                    Ok(()) => this
                        .settings
                        .finish_apply("Transfer settings saved.".to_string()),
                    Err(error) => this
                        .settings
                        .fail_apply(format!("Couldn't save transfer settings: {error}")),
                }
                cx.notify();
            });
            Ok::<_, anyhow::Error>(())
        })
        .detach();
    }

    pub(super) fn request_apply_storage_settings(&mut self, cx: &mut Context<Self>) {
        let Some(page) = self.current_settings_page(cx) else {
            self.settings
                .fail_apply("Settings are still loading.".to_string());
            cx.notify();
            return;
        };

        let input = match self.parse_storage_settings_input(page.storage.gc_batch_size, cx) {
            Ok(input) => input,
            Err(message) => {
                self.settings.fail_apply(message);
                cx.notify();
                return;
            }
        };

        let Some(task) = settings_actions::set_storage_settings_task(&self.controller, input, cx)
        else {
            self.settings
                .fail_apply("Settings are still loading.".to_string());
            cx.notify();
            return;
        };

        self.settings.begin_apply(
            SettingsSectionKey::Storage,
            "Saving storage settings.".to_string(),
        );
        cx.notify();

        let view = cx.entity().downgrade();
        cx.spawn(async move |_, cx| {
            let result = task.await;
            let _ = view.update(cx, |this, cx| {
                match result {
                    Ok(()) => this
                        .settings
                        .finish_apply("Storage settings saved.".to_string()),
                    Err(error) => this
                        .settings
                        .fail_apply(format!("Couldn't save storage settings: {error}")),
                }
                cx.notify();
            });
            Ok::<_, anyhow::Error>(())
        })
        .detach();
    }

    fn parse_storage_settings_input(
        &self,
        current_gc_batch_size: usize,
        cx: &Context<Self>,
    ) -> Result<StorageSettingsInput, String> {
        let history_window_days = self.settings.history_window_days(cx)?;
        let dedup_window_days = self.settings.dedup_window_days(cx)?;
        let max_text_bytes = self.settings.max_text_bytes(cx)?;

        if dedup_window_days < history_window_days {
            return Err(
                "Duplicate check window must be at least as long as history retention."
                    .to_string(),
            );
        }

        Ok(StorageSettingsInput {
            history_window_days,
            dedup_window_days,
            max_text_bytes,
            gc_batch_size: current_gc_batch_size,
        })
    }

    fn current_settings_page(&self, cx: &Context<Self>) -> Option<SettingsPageViewState> {
        self.render_model(cx).page.map(|page| page.settings)
    }
}
