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

use super::state::SettingsSectionKey;

impl WorkspaceView {
    pub(super) fn copy_settings_config_path(&mut self, path: String, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(path));
        self.settings
            .set_feedback("Copied config path to clipboard.".to_string());
        cx.notify();
    }

    pub(super) fn request_settings_toggle_lan_enabled(&mut self, cx: &mut Context<Self>) {
        self.settings.toggle_lan_enabled();
        cx.notify();
    }

    pub(super) fn request_settings_toggle_local_capture(&mut self, cx: &mut Context<Self>) {
        self.settings.toggle_local_capture_enabled();
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
                .fail_apply("Settings snapshot is not ready yet.".to_string());
            cx.notify();
            return;
        };
        let Ok(listen_port) = self.settings.listen_port_value(cx).trim().parse::<u16>() else {
            self.settings
                .fail_apply("Listen port must be a valid u16 value.".to_string());
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
                .fail_apply("Settings core bridge is not ready yet.".to_string());
            cx.notify();
            return;
        };

        self.settings.begin_apply(
            SettingsSectionKey::Connection,
            "Applying connection settings.".to_string(),
        );
        cx.notify();

        let view = cx.entity().downgrade();
        cx.spawn(async move |_, cx| {
            let result = task.await;
            let _ = view.update(cx, |this, cx| {
                match result {
                    Ok(()) => this
                        .settings
                        .finish_apply("Applied connection settings.".to_string()),
                    Err(error) => this
                        .settings
                        .fail_apply(format!("Failed to apply connection settings: {error}")),
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
                .fail_apply("Settings snapshot is not ready yet.".to_string());
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
                .fail_apply("Settings core bridge is not ready yet.".to_string());
            cx.notify();
            return;
        };

        self.settings.begin_apply(
            SettingsSectionKey::Clipboard,
            "Applying clipboard settings.".to_string(),
        );
        cx.notify();

        let view = cx.entity().downgrade();
        cx.spawn(async move |_, cx| {
            let result = task.await;
            let _ = view.update(cx, |this, cx| {
                match result {
                    Ok(()) => this
                        .settings
                        .finish_apply("Applied clipboard settings.".to_string()),
                    Err(error) => this
                        .settings
                        .fail_apply(format!("Failed to apply clipboard settings: {error}")),
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
                .fail_apply("Settings core bridge is not ready yet.".to_string());
            cx.notify();
            return;
        };

        self.settings.begin_apply(
            SettingsSectionKey::Transfers,
            "Applying transfer settings.".to_string(),
        );
        cx.notify();

        let view = cx.entity().downgrade();
        cx.spawn(async move |_, cx| {
            let result = task.await;
            let _ = view.update(cx, |this, cx| {
                match result {
                    Ok(()) => this
                        .settings
                        .finish_apply("Applied transfer settings.".to_string()),
                    Err(error) => this
                        .settings
                        .fail_apply(format!("Failed to apply transfer settings: {error}")),
                }
                cx.notify();
            });
            Ok::<_, anyhow::Error>(())
        })
        .detach();
    }

    pub(super) fn request_apply_storage_settings(&mut self, cx: &mut Context<Self>) {
        let input = match self.parse_storage_settings_input(cx) {
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
                .fail_apply("Settings core bridge is not ready yet.".to_string());
            cx.notify();
            return;
        };

        self.settings.begin_apply(
            SettingsSectionKey::Storage,
            "Applying storage settings.".to_string(),
        );
        cx.notify();

        let view = cx.entity().downgrade();
        cx.spawn(async move |_, cx| {
            let result = task.await;
            let _ = view.update(cx, |this, cx| {
                match result {
                    Ok(()) => this
                        .settings
                        .finish_apply("Applied storage settings.".to_string()),
                    Err(error) => this
                        .settings
                        .fail_apply(format!("Failed to apply storage settings: {error}")),
                }
                cx.notify();
            });
            Ok::<_, anyhow::Error>(())
        })
        .detach();
    }

    fn parse_storage_settings_input(
        &self,
        cx: &Context<Self>,
    ) -> Result<StorageSettingsInput, String> {
        Ok(StorageSettingsInput {
            history_window_days: self
                .settings
                .history_window_value(cx)
                .trim()
                .parse::<u32>()
                .map_err(|_| "History window must be a valid u32 value.".to_string())?,
            dedup_window_days: self
                .settings
                .dedup_window_value(cx)
                .trim()
                .parse::<u32>()
                .map_err(|_| "Dedup window must be a valid u32 value.".to_string())?,
            max_text_bytes: self
                .settings
                .max_text_bytes_value(cx)
                .trim()
                .parse::<usize>()
                .map_err(|_| "Max text bytes must be a valid usize value.".to_string())?,
            gc_batch_size: self
                .settings
                .gc_batch_size_value(cx)
                .trim()
                .parse::<usize>()
                .map_err(|_| "GC batch size must be a valid usize value.".to_string())?,
        })
    }

    fn current_settings_page(&self, cx: &Context<Self>) -> Option<SettingsPageViewState> {
        self.render_model(cx).page.map(|page| page.settings)
    }
}
