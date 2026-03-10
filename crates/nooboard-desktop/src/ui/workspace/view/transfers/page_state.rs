use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Error;
use gpui::{Context, PathPromptOptions, Window};
use nooboard_app::{
    IncomingTransferDecision, IncomingTransferDisposition, NoobId, SendFileItem, SendFilesRequest,
    TransferId,
};

use crate::state::{WorkspaceRoute, live_commands};

use super::WorkspaceView;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum StagedFileSource {
    Dropped,
    Browsed,
}

impl StagedFileSource {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Dropped => "Dropped",
            Self::Browsed => "Browsed",
        }
    }
}

#[derive(Clone)]
pub(super) struct StagedTransferFile {
    pub(super) id: String,
    pub(super) file_name: String,
    pub(super) file_path: PathBuf,
    pub(super) size_bytes: u64,
    pub(super) size_label: String,
    pub(super) modified_at_label: String,
    pub(super) source: StagedFileSource,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TransfersSendState {
    Idle,
    Sending,
}

pub(in crate::ui::workspace::view) struct TransfersPageState {
    pub(super) selected_target_noob_ids: BTreeSet<String>,
    pub(super) staged_files: Vec<StagedTransferFile>,
    pub(super) pending_transfer_actions: BTreeSet<String>,
    pub(super) send_state: TransfersSendState,
    pub(super) feedback: Option<String>,
    next_staged_file_id: usize,
}

impl TransfersPageState {
    pub(in crate::ui::workspace::view) fn new() -> Self {
        Self {
            selected_target_noob_ids: BTreeSet::new(),
            staged_files: Vec::new(),
            pending_transfer_actions: BTreeSet::new(),
            send_state: TransfersSendState::Idle,
            feedback: None,
            next_staged_file_id: 1,
        }
    }

    pub(super) fn selected_target_count(&self) -> usize {
        self.selected_target_noob_ids.len()
    }

    pub(super) fn staged_file_count(&self) -> usize {
        self.staged_files.len()
    }

    pub(super) fn send_in_flight(&self) -> bool {
        self.send_state == TransfersSendState::Sending
    }

    pub(super) fn transfer_action_pending(&self, transfer_id: &TransferId) -> bool {
        self.pending_transfer_actions
            .contains(&transfer_id.to_string())
    }

    pub(super) fn retain_connected_targets(
        &mut self,
        connected_target_noob_ids: &BTreeSet<String>,
    ) {
        self.selected_target_noob_ids
            .retain(|noob_id| connected_target_noob_ids.contains(noob_id));
    }

    fn next_staged_file_id(&mut self) -> String {
        let id = format!("transfer-stage-{}", self.next_staged_file_id);
        self.next_staged_file_id += 1;
        id
    }
}

impl WorkspaceView {
    pub(super) fn set_transfers_feedback(&mut self, message: impl Into<String>) {
        self.transfers_page_state.feedback = Some(message.into());
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

    pub(super) fn queue_upload_paths(
        &mut self,
        paths: Vec<PathBuf>,
        source: StagedFileSource,
        cx: &mut Context<Self>,
    ) {
        let mut existing = self
            .transfers_page_state
            .staged_files
            .iter()
            .map(|item| item.file_path.clone())
            .collect::<BTreeSet<_>>();
        let mut staged_now = 0usize;

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
            let staged_file_id = self.transfers_page_state.next_staged_file_id();
            self.transfers_page_state
                .staged_files
                .push(StagedTransferFile {
                    id: staged_file_id,
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
                });
            existing.insert(path);
            staged_now += 1;
        }

        if staged_now > 0 {
            self.set_transfers_feedback(format!(
                "Staged {} file{} for transfer.",
                staged_now,
                if staged_now == 1 { "" } else { "s" }
            ));
            cx.notify();
        }
    }

    pub(super) fn dismiss_staged_file(&mut self, staged_file_id: &str, cx: &mut Context<Self>) {
        let before = self.transfers_page_state.staged_files.len();
        self.transfers_page_state
            .staged_files
            .retain(|item| item.id != staged_file_id);
        if self.transfers_page_state.staged_files.len() != before {
            self.set_transfers_feedback("Removed staged file.");
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
            prompt: Some("Select files to transfer".into()),
        });
        let view = cx.entity().downgrade();

        cx.spawn_in(window, async move |_, cx| {
            let paths = match paths_receiver.await {
                Ok(Ok(Some(paths))) => paths,
                _ => return,
            };

            let _ = view.update(cx, |this, cx| {
                this.queue_upload_paths(paths, StagedFileSource::Browsed, cx);
            });
        })
        .detach();
    }

    pub(super) fn submit_staged_transfers(&mut self, cx: &mut Context<Self>) {
        if self.transfers_page_state.send_in_flight() {
            return;
        }

        if self.transfers_page_state.staged_files.is_empty() {
            self.set_transfers_feedback("Stage at least one file before sending.");
            cx.notify();
            return;
        }

        if self
            .transfers_page_state
            .selected_target_noob_ids
            .is_empty()
        {
            self.set_transfers_feedback("Select at least one connected target.");
            cx.notify();
            return;
        }

        let target_ids = self
            .transfers_page_state
            .selected_target_noob_ids
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        let staged_files = self.transfers_page_state.staged_files.clone();
        let commands = live_commands::client(cx);
        let view = cx.entity().downgrade();

        self.transfers_page_state.send_state = TransfersSendState::Sending;
        self.set_transfers_feedback("Submitting staged files to the app service.");
        cx.notify();

        cx.spawn(async move |_, cx| {
            let request = SendFilesRequest {
                targets: target_ids.iter().cloned().map(NoobId::new).collect(),
                files: staged_files
                    .iter()
                    .map(|item| SendFileItem {
                        path: item.file_path.clone(),
                    })
                    .collect(),
            };

            let target_count = target_ids.len();
            let file_count = staged_files.len();

            match commands.send_files(request, cx).await {
                Ok(_) => {
                    let _ = view.update(cx, |this, cx| {
                        this.transfers_page_state.send_state = TransfersSendState::Idle;
                        this.transfers_page_state.staged_files.clear();
                        this.set_transfers_feedback(format!(
                            "Submitted {} file{} to {} target{}.",
                            file_count,
                            if file_count == 1 { "" } else { "s" },
                            target_count,
                            if target_count == 1 { "" } else { "s" }
                        ));
                        cx.notify();
                    });
                }
                Err(error) => {
                    let _ = view.update(cx, |this, cx| {
                        this.transfers_page_state.send_state = TransfersSendState::Idle;
                        this.set_transfers_feedback(format!(
                            "Failed to submit staged files: {error}"
                        ));
                        cx.notify();
                    });
                }
            }

            Ok::<_, Error>(())
        })
        .detach();
    }

    pub(super) fn request_incoming_transfer_decision(
        &mut self,
        transfer_id: TransferId,
        decision: IncomingTransferDisposition,
        cx: &mut Context<Self>,
    ) {
        let action_key = transfer_id.to_string();
        if !self
            .transfers_page_state
            .pending_transfer_actions
            .insert(action_key.clone())
        {
            return;
        }

        let commands = live_commands::client(cx);
        let view = cx.entity().downgrade();
        let detail = match decision {
            IncomingTransferDisposition::Accept => "Accepting incoming transfer.",
            IncomingTransferDisposition::Reject => "Rejecting incoming transfer.",
        };
        self.set_transfers_feedback(detail);
        cx.notify();

        cx.spawn(async move |_, cx| {
            let request = IncomingTransferDecision {
                transfer_id: transfer_id.clone(),
                decision: decision.clone(),
            };

            let result = commands.decide_incoming_transfer(request, cx).await;
            let _ = view.update(cx, |this, cx| {
                this.transfers_page_state
                    .pending_transfer_actions
                    .remove(&action_key);
                match result {
                    Ok(()) => {
                        this.set_transfers_feedback(match decision {
                            IncomingTransferDisposition::Accept => {
                                "Accepted incoming transfer. Waiting for app state."
                            }
                            IncomingTransferDisposition::Reject => {
                                "Rejected incoming transfer. Waiting for app state."
                            }
                        });
                    }
                    Err(error) => {
                        this.set_transfers_feedback(format!(
                            "Failed to update incoming transfer {}: {error}",
                            transfer_id
                        ));
                    }
                }
                cx.notify();
            });

            Ok::<_, Error>(())
        })
        .detach();
    }

    pub(super) fn request_cancel_transfer(
        &mut self,
        transfer_id: TransferId,
        cx: &mut Context<Self>,
    ) {
        let action_key = transfer_id.to_string();
        if !self
            .transfers_page_state
            .pending_transfer_actions
            .insert(action_key.clone())
        {
            return;
        }

        let commands = live_commands::client(cx);
        let view = cx.entity().downgrade();
        self.set_transfers_feedback("Cancelling transfer.");
        cx.notify();

        cx.spawn(async move |_, cx| {
            let result = commands.cancel_transfer(transfer_id.clone(), cx).await;
            let _ = view.update(cx, |this, cx| {
                this.transfers_page_state
                    .pending_transfer_actions
                    .remove(&action_key);
                match result {
                    Ok(()) => {
                        this.set_transfers_feedback("Cancel requested. Waiting for app state.");
                    }
                    Err(error) => {
                        this.set_transfers_feedback(format!(
                            "Failed to cancel transfer {}: {error}",
                            transfer_id
                        ));
                    }
                }
                cx.notify();
            });

            Ok::<_, Error>(())
        })
        .detach();
    }

    pub(super) fn open_transfer_settings(
        &mut self,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        self.request_workspace_route(WorkspaceRoute::Settings, window, cx);
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

fn system_time_to_clock_label(time: SystemTime) -> String {
    let seconds = time
        .duration_since(UNIX_EPOCH)
        .map(|duration| i64::try_from(duration.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or(0)
        .rem_euclid(86_400);
    let hour = seconds / 3_600;
    let minute = (seconds % 3_600) / 60;
    let second = seconds % 60;
    format!("{hour:02}:{minute:02}:{second:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retain_connected_targets_prunes_stale_selection() {
        let mut page_state = TransfersPageState::new();
        page_state
            .selected_target_noob_ids
            .extend(["peer-a".to_string(), "peer-b".to_string()]);

        page_state.retain_connected_targets(&BTreeSet::from(["peer-b".to_string()]));

        assert_eq!(
            page_state.selected_target_noob_ids,
            BTreeSet::from(["peer-b".to_string()])
        );
    }
}
