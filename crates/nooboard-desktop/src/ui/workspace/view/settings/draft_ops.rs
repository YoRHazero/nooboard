use gpui::{ClipboardItem, Context, PathPromptOptions, Window};

use super::patches::{add_manual_peer, parse_manual_peer_input};
use super::{StorageSettingField, WorkspaceView};

impl WorkspaceView {
    pub(super) fn reset_network_settings_draft(&mut self, cx: &mut Context<Self>) {
        self.settings_page_state.network.reset();
        self.clear_settings_feedback();
        cx.notify();
    }

    pub(super) fn reset_storage_settings_draft(&mut self, cx: &mut Context<Self>) {
        self.settings_page_state.storage.reset();
        self.clear_settings_feedback();
        cx.notify();
    }

    pub(super) fn reset_clipboard_settings_draft(&mut self, cx: &mut Context<Self>) {
        self.settings_page_state.clipboard.reset();
        self.clear_settings_feedback();
        cx.notify();
    }

    pub(super) fn reset_transfer_settings_draft(&mut self, cx: &mut Context<Self>) {
        self.settings_page_state.transfers.reset();
        self.clear_settings_feedback();
        cx.notify();
    }

    pub(super) fn toggle_settings_network_enabled(&mut self, cx: &mut Context<Self>) {
        self.settings_page_state.network.draft.network_enabled =
            !self.settings_page_state.network.draft.network_enabled;
        self.settings_page_state.network.mark_edited();
        self.clear_settings_feedback();
        cx.notify();
    }

    pub(super) fn toggle_settings_mdns_enabled(&mut self, cx: &mut Context<Self>) {
        self.settings_page_state.network.draft.mdns_enabled =
            !self.settings_page_state.network.draft.mdns_enabled;
        self.settings_page_state.network.mark_edited();
        self.clear_settings_feedback();
        cx.notify();
    }

    pub(super) fn toggle_settings_token_visibility(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.settings_page_state.token_visible = !self.settings_page_state.token_visible;
        let masked = !self.settings_page_state.token_visible;
        let _ = self
            .settings_page_state
            .token_input
            .update(cx, |input, cx| {
                input.set_masked(masked, window, cx);
            });
        cx.notify();
    }

    pub(super) fn toggle_settings_local_capture_enabled(&mut self, cx: &mut Context<Self>) {
        self.settings_page_state
            .clipboard
            .draft
            .local_capture_enabled = !self
            .settings_page_state
            .clipboard
            .draft
            .local_capture_enabled;
        self.settings_page_state.clipboard.mark_edited();
        self.clear_settings_feedback();
        cx.notify();
    }

    pub(super) fn step_storage_setting(
        &mut self,
        field: StorageSettingField,
        increment: bool,
        cx: &mut Context<Self>,
    ) {
        let storage = &mut self.settings_page_state.storage.draft;

        match field {
            StorageSettingField::HistoryWindowDays => {
                let step = field.step() as u32;
                storage.history_window_days = if increment {
                    storage.history_window_days.saturating_add(step)
                } else {
                    storage.history_window_days.saturating_sub(step).max(1)
                };
                if storage.dedup_window_days < storage.history_window_days {
                    storage.dedup_window_days = storage.history_window_days;
                }
            }
            StorageSettingField::DedupWindowDays => {
                let step = field.step() as u32;
                storage.dedup_window_days = if increment {
                    storage.dedup_window_days.saturating_add(step)
                } else {
                    storage.dedup_window_days.saturating_sub(step).max(1)
                };
            }
            StorageSettingField::MaxTextBytes => {
                let step = field.step();
                storage.max_text_bytes = if increment {
                    storage.max_text_bytes.saturating_add(step)
                } else {
                    storage.max_text_bytes.saturating_sub(step).max(1)
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

        self.settings_page_state.storage.mark_edited();
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
                this.settings_page_state.storage.draft.db_root = path;
                this.settings_page_state.storage.mark_edited();
                this.clear_settings_feedback();
                cx.notify();
            });
        })
        .detach();
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

        cx.spawn_in(window, async move |_, cx| {
            let path = match paths_receiver.await {
                Ok(Ok(Some(mut paths))) => paths.drain(..).next(),
                _ => None,
            };

            let Some(path) = path else {
                return;
            };

            let _ = view.update(cx, |this, cx| {
                this.settings_page_state.transfers.draft.download_dir = path;
                this.settings_page_state.transfers.mark_edited();
                this.clear_settings_feedback();
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn commit_settings_manual_peer(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let input = self
            .settings_page_state
            .manual_peer_input
            .read(cx)
            .value()
            .to_string();
        let addr = match parse_manual_peer_input(&input) {
            Ok(addr) => addr,
            Err(message) => {
                self.set_settings_feedback(message);
                cx.notify();
                return;
            }
        };

        match add_manual_peer(
            &mut self.settings_page_state.network.draft.manual_peers,
            addr,
        ) {
            Ok(()) => {
                self.settings_page_state.network.mark_edited();
                self.clear_settings_feedback();
                let _ = self
                    .settings_page_state
                    .manual_peer_input
                    .update(cx, |input_state, cx| {
                        input_state.set_value("", window, cx);
                    });
                cx.notify();
            }
            Err(message) => {
                self.set_settings_feedback(message);
                cx.notify();
            }
        }
    }

    pub(super) fn remove_settings_manual_peer(
        &mut self,
        addr: std::net::SocketAddr,
        cx: &mut Context<Self>,
    ) {
        let before = self.settings_page_state.network.draft.manual_peers.len();
        self.settings_page_state
            .network
            .draft
            .manual_peers
            .retain(|item| *item != addr);

        if self.settings_page_state.network.draft.manual_peers.len() != before {
            self.settings_page_state.network.mark_edited();
            self.clear_settings_feedback();
            cx.notify();
        }
    }

    pub(super) fn copy_settings_device_endpoint(&mut self, cx: &mut Context<Self>) {
        let Some(endpoint) = self.network_device_endpoint_preview() else {
            self.set_settings_feedback(
                "No shareable device endpoint is available until app detects a local IPv4 and a valid port.",
            );
            cx.notify();
            return;
        };

        cx.write_to_clipboard(ClipboardItem::new_string(endpoint.clone()));
        self.set_settings_feedback(format!("Copied {endpoint}."));
        cx.notify();
    }
}
