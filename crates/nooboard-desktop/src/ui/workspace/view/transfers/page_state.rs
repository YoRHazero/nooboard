use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use gpui::{Context, PathPromptOptions, Window};
use gpui_component::WindowExt;

use crate::state::{ClipboardStore, ClipboardTarget, TransferStatus};

use super::WorkspaceView;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum UploadSource {
    Dropped,
    Browsed,
}

impl UploadSource {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Dropped => "Dropped",
            Self::Browsed => "Browsed",
        }
    }
}

#[derive(Clone)]
pub(super) enum LocalUploadStatus {
    Draft,
    Accepted {
        at_label: String,
        accepted_targets: usize,
    },
    Rejected {
        at_label: String,
        reason: String,
    },
    Progress {
        progress: f32,
        speed_label: String,
        eta_label: String,
    },
    Complete {
        at_label: String,
    },
}

#[derive(Clone)]
pub(super) struct LocalUploadCard {
    pub(super) id: String,
    pub(super) file_name: String,
    pub(super) file_path: PathBuf,
    pub(super) size_bytes: u64,
    pub(super) size_label: String,
    pub(super) modified_at_label: String,
    pub(super) source: UploadSource,
    pub(super) status: LocalUploadStatus,
    pub(super) sent_target_ids: Vec<String>,
}

pub(in crate::ui::workspace::view) struct TransfersPageState {
    pub(super) selected_target_noob_ids: BTreeSet<String>,
    pub(super) global_folder: PathBuf,
    pub(super) uploads: Vec<LocalUploadCard>,
    pub(super) feedback: Option<String>,
    pub(super) moved_download_paths: BTreeMap<String, PathBuf>,
    next_upload_id: usize,
    send_cycle: usize,
}

impl TransfersPageState {
    pub(in crate::ui::workspace::view) fn new(clipboard: &ClipboardStore) -> Self {
        Self {
            selected_target_noob_ids: clipboard
                .default_selected_target_noob_ids
                .iter()
                .cloned()
                .collect(),
            global_folder: PathBuf::from(".dev-data/downloads"),
            uploads: Vec::new(),
            feedback: None,
            moved_download_paths: BTreeMap::new(),
            next_upload_id: 1,
            send_cycle: 0,
        }
    }

    pub(super) fn selected_target_count(&self) -> usize {
        self.selected_target_noob_ids.len()
    }

    fn next_upload_id(&mut self) -> String {
        let id = format!("transfer-upload-{}", self.next_upload_id);
        self.next_upload_id += 1;
        id
    }
}

impl WorkspaceView {
    pub(super) fn set_transfers_feedback(&mut self, message: impl Into<String>) {
        self.transfers_page_state.feedback = Some(message.into());
    }

    pub(super) fn transfer_target_is_selected(&self, noob_id: &str) -> bool {
        self.transfers_page_state
            .selected_target_noob_ids
            .contains(noob_id)
    }

    pub(super) fn selected_transfer_target_count(&self) -> usize {
        self.transfers_page_state.selected_target_count()
    }

    pub(super) fn toggle_transfer_target(&mut self, noob_id: &str, cx: &mut Context<Self>) {
        if self
            .transfers_page_state
            .selected_target_noob_ids
            .contains(noob_id)
        {
            self.transfers_page_state
                .selected_target_noob_ids
                .remove(noob_id);
        } else {
            self.transfers_page_state
                .selected_target_noob_ids
                .insert(noob_id.to_string());
        }

        let count = self.transfers_page_state.selected_target_count();
        self.set_transfers_feedback(format!(
            "{} transfer target{} selected.",
            count,
            if count == 1 { "" } else { "s" }
        ));
        cx.notify();
    }

    pub(super) fn transfer_selected_targets(&self) -> Vec<ClipboardTarget> {
        self.state
            .app
            .clipboard
            .targets
            .iter()
            .filter(|target| {
                target.is_connected()
                    && self
                        .transfers_page_state
                        .selected_target_noob_ids
                        .contains(&target.noob_id)
            })
            .cloned()
            .collect()
    }

    pub(super) fn queue_upload_paths(
        &mut self,
        paths: Vec<PathBuf>,
        source: UploadSource,
        cx: &mut Context<Self>,
    ) {
        let mut added = 0usize;
        let existing: BTreeSet<PathBuf> = self
            .transfers_page_state
            .uploads
            .iter()
            .map(|item| item.file_path.clone())
            .collect();

        for path in paths {
            if existing.contains(&path) {
                continue;
            }

            let Some(metadata) = fs::metadata(&path).ok() else {
                continue;
            };
            if !metadata.is_file() {
                continue;
            }
            let Some(file_name) = path.file_name() else {
                continue;
            };

            let card = LocalUploadCard {
                id: self.transfers_page_state.next_upload_id(),
                file_name: file_name.to_string_lossy().into_owned(),
                file_path: path.clone(),
                size_bytes: metadata.len(),
                size_label: bytes_to_label(metadata.len()),
                modified_at_label: metadata
                    .modified()
                    .ok()
                    .map(system_time_to_clock_label)
                    .unwrap_or_else(|| "unknown".to_string()),
                source,
                status: LocalUploadStatus::Draft,
                sent_target_ids: Vec::new(),
            };
            self.transfers_page_state.uploads.push(card);
            added += 1;
        }

        if added > 0 {
            self.set_transfers_feedback(format!(
                "{} file{} queued for upload review.",
                added,
                if added == 1 { "" } else { "s" }
            ));
            cx.notify();
        }
    }

    pub(super) fn pick_transfer_upload_files(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let paths_receiver = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some("Select files to upload".into()),
        });
        let view = cx.entity().downgrade();

        cx.spawn_in(window, async move |_, cx| {
            let paths = match paths_receiver.await {
                Ok(Ok(Some(paths))) => paths,
                _ => return,
            };

            let _ = view.update(cx, |this, cx| {
                this.queue_upload_paths(paths, UploadSource::Browsed, cx);
            });
        })
        .detach();
    }

    pub(super) fn pick_transfer_global_folder(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let paths_receiver = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Select transfer folder".into()),
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
                this.transfers_page_state.global_folder = path.clone();
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn set_transfer_global_folder(&mut self, folder: PathBuf, cx: &mut Context<Self>) {
        self.transfers_page_state.global_folder = folder;
        cx.notify();
    }

    pub(super) fn request_send_local_upload(
        &mut self,
        upload_id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let selected_targets = self.transfer_selected_targets();
        if selected_targets.is_empty() {
            self.set_transfers_feedback("Select at least one connected target.");
            cx.notify();
            return;
        }

        let Some(card) = self
            .transfers_page_state
            .uploads
            .iter()
            .find(|item| item.id == upload_id)
        else {
            return;
        };

        let file_name = card.file_name.clone();
        let item_id = card.id.clone();
        let target_lines = selected_targets
            .iter()
            .map(|target| format!("• {} ({})", target.device_id, target.noob_id))
            .collect::<Vec<_>>()
            .join("\n");
        let description = format!(
            "Upload \"{}\" to {} selected target{}?\n\n{}",
            file_name,
            selected_targets.len(),
            if selected_targets.len() == 1 { "" } else { "s" },
            target_lines
        );
        let view = cx.entity().downgrade();

        window.open_alert_dialog(cx, move |alert, _, _| {
            let view = view.clone();
            let item_id = item_id.clone();
            let description = description.clone();

            alert
                .confirm()
                .title("Confirm File Upload")
                .description(description)
                .on_ok(move |_, _, cx| {
                    let _ = view.update(cx, |this, cx| {
                        this.confirm_send_local_upload(item_id.as_str(), cx);
                    });
                    true
                })
        });
    }

    pub(super) fn confirm_send_local_upload(&mut self, upload_id: &str, cx: &mut Context<Self>) {
        let selected_target_ids = self
            .transfer_selected_targets()
            .into_iter()
            .map(|target| target.noob_id)
            .collect::<Vec<_>>();
        let target_count = selected_target_ids.len();

        let at_label = now_clock_label();
        let slot = self.transfers_page_state.send_cycle % 4;
        self.transfers_page_state.send_cycle += 1;

        let file_name = {
            let Some(card) = self
                .transfers_page_state
                .uploads
                .iter_mut()
                .find(|item| item.id == upload_id)
            else {
                return;
            };

            card.sent_target_ids = selected_target_ids;
            card.status = match slot {
                0 => LocalUploadStatus::Accepted {
                    at_label: at_label.clone(),
                    accepted_targets: target_count,
                },
                1 => LocalUploadStatus::Rejected {
                    at_label: at_label.clone(),
                    reason: "Remote policy denied this upload".to_string(),
                },
                2 => LocalUploadStatus::Progress {
                    progress: 0.46,
                    speed_label: "2.8 MB/s".to_string(),
                    eta_label: "ETA 11s".to_string(),
                },
                _ => LocalUploadStatus::Complete {
                    at_label: at_label.clone(),
                },
            };
            card.file_name.clone()
        };

        self.set_transfers_feedback(format!("{} dispatch queued.", file_name));
        cx.notify();
    }

    pub(super) fn dismiss_local_upload(&mut self, upload_id: &str, cx: &mut Context<Self>) {
        let before = self.transfers_page_state.uploads.len();
        self.transfers_page_state
            .uploads
            .retain(|item| item.id != upload_id);
        if self.transfers_page_state.uploads.len() != before {
            self.set_transfers_feedback("Local upload card dismissed.");
            cx.notify();
        }
    }

    pub(super) fn accept_download_transfer(&mut self, item_id: &str, cx: &mut Context<Self>) {
        let mut changed = false;
        for item in &mut self.transfer_items {
            if item.id != item_id {
                continue;
            }
            if matches!(item.status, TransferStatus::AwaitingReview { .. }) {
                item.status = TransferStatus::Progress {
                    progress: 0.18,
                    speed_label: "2.4 MB/s".to_string(),
                    started_at_label: now_clock_label(),
                    elapsed_label: "8s".to_string(),
                    eta_label: "ETA 36s".to_string(),
                };
                changed = true;
            }
            break;
        }

        if changed {
            self.set_transfers_feedback("Download accepted and transfer started.");
            cx.notify();
        }
    }

    pub(super) fn reject_download_transfer(&mut self, item_id: &str, cx: &mut Context<Self>) {
        let before = self.transfer_items.len();
        self.transfer_items.retain(|item| {
            !(item.id == item_id && matches!(item.status, TransferStatus::AwaitingReview { .. }))
        });
        if self.transfer_items.len() != before {
            self.set_transfers_feedback("Incoming file rejected.");
            cx.notify();
        }
    }

    pub(super) fn cancel_download_transfer(&mut self, item_id: &str, cx: &mut Context<Self>) {
        let before = self.transfer_items.len();
        self.transfer_items.retain(|item| {
            !(item.id == item_id && matches!(item.status, TransferStatus::Progress { .. }))
        });
        if self.transfer_items.len() != before {
            self.set_transfers_feedback("In-progress transfer canceled.");
            cx.notify();
        }
    }

    pub(super) fn got_it_download_transfer(&mut self, item_id: &str, cx: &mut Context<Self>) {
        let before = self.transfer_items.len();
        self.transfer_items.retain(|item| {
            !(item.id == item_id && matches!(item.status, TransferStatus::Complete { .. }))
        });
        if self.transfer_items.len() != before {
            self.transfers_page_state
                .moved_download_paths
                .remove(item_id);
            self.set_transfers_feedback("Completed transfer dismissed.");
            cx.notify();
        }
    }

    pub(super) fn move_complete_download_transfer(
        &mut self,
        item_id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let paths_receiver = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Select destination folder".into()),
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
                this.transfers_page_state
                    .moved_download_paths
                    .insert(item_id.clone(), path.clone());
                this.set_transfers_feedback(format!("Move scheduled to {}.", path.display()));
                cx.notify();
            });
        })
        .detach();
    }
}

fn bytes_to_label(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;

    let bytes_f = bytes as f64;
    if bytes_f >= GB {
        format!("{:.1} GB", bytes_f / GB)
    } else if bytes_f >= MB {
        format!("{:.1} MB", bytes_f / MB)
    } else if bytes_f >= KB {
        format!("{:.1} KB", bytes_f / KB)
    } else {
        format!("{} B", bytes)
    }
}

fn now_clock_label() -> String {
    system_time_to_clock_label(SystemTime::now())
}

fn system_time_to_clock_label(time: SystemTime) -> String {
    let seconds = time
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() % 86_400)
        .unwrap_or(0);

    let hour = seconds / 3_600;
    let minute = (seconds % 3_600) / 60;
    let second = seconds % 60;
    format!("{:02}:{:02}:{:02}", hour, minute, second)
}
