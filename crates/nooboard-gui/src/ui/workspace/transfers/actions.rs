use std::path::PathBuf;

use gpui::{Context, PathPromptOptions, Window};
use nooboard_core::{
    IncomingTransferDecision, IncomingTransferDisposition, SendFilesRequest, SessionTarget,
};

use crate::{
    ui::workspace::WorkspaceView,
    workspace::{actions::transfers as transfer_actions, view_state::IncomingTransferViewState},
};

use super::state::StagedFileSource;

impl WorkspaceView {
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
                this.transfers.queue_paths(paths, StagedFileSource::Browsed);
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn queue_transfer_drop_paths(
        &mut self,
        paths: Vec<PathBuf>,
        cx: &mut Context<Self>,
    ) {
        self.transfers.queue_paths(paths, StagedFileSource::Dropped);
        cx.notify();
    }

    pub(super) fn request_transfer_toggle_target(
        &mut self,
        id: nooboard_core::SessionId,
        cx: &mut Context<Self>,
    ) {
        self.transfers.toggle_target(id);
        cx.notify();
    }

    pub(super) fn request_transfer_remove_staged(
        &mut self,
        staged_file_id: &str,
        cx: &mut Context<Self>,
    ) {
        self.transfers.remove_staged_file(staged_file_id);
        cx.notify();
    }

    pub(super) fn request_transfer_send(&mut self, cx: &mut Context<Self>) {
        if self.transfers.send_in_flight() {
            return;
        }
        if self.transfers.staged_files().is_empty() {
            self.transfers
                .fail_send("Add at least one file before sending.".to_string());
            cx.notify();
            return;
        }
        if self.transfers.selected_session_ids().is_empty() {
            self.transfers
                .fail_send("Choose at least one connected device.".to_string());
            cx.notify();
            return;
        }

        let Some(task) = transfer_actions::send_files_task(
            &self.controller,
            SendFilesRequest {
                files: self
                    .transfers
                    .staged_files()
                    .iter()
                    .map(|item| item.file_path.clone())
                    .collect(),
                target: SessionTarget::Sessions(
                    self.transfers
                        .selected_session_ids()
                        .iter()
                        .copied()
                        .collect(),
                ),
            },
            cx,
        ) else {
            self.transfers
                .fail_send("Transfers are still loading.".to_string());
            cx.notify();
            return;
        };

        self.transfers.begin_send();
        cx.notify();

        let view = cx.entity().downgrade();
        cx.spawn(async move |_, cx| {
            let result = task.await;
            let _ = view.update(cx, |this, cx| {
                match result {
                    Ok(tickets) => this
                        .transfers
                        .finish_send(format!("Started {} transfer(s).", tickets.len())),
                    Err(error) => this
                        .transfers
                        .fail_send(format!("Couldn't send files: {error}")),
                }
                cx.notify();
            });
            Ok::<_, anyhow::Error>(())
        })
        .detach();
    }

    pub(super) fn request_transfer_decision(
        &mut self,
        transfer: &IncomingTransferViewState,
        accept: bool,
        cx: &mut Context<Self>,
    ) {
        let decision = IncomingTransferDecision {
            ticket: transfer.ticket,
            decision: if accept {
                IncomingTransferDisposition::Accept
            } else {
                IncomingTransferDisposition::Reject
            },
        };
        let Some(task) =
            transfer_actions::decide_incoming_transfer_task(&self.controller, decision, cx)
        else {
            self.transfers.clear_ticket_pending(
                transfer.ticket,
                "Transfers are still loading.".to_string(),
            );
            cx.notify();
            return;
        };

        self.transfers.mark_ticket_pending(
            transfer.ticket,
            format!(
                "{} incoming transfer '{}'.",
                if accept { "Accepting" } else { "Rejecting" },
                transfer.file_name
            ),
        );
        cx.notify();

        let ticket = transfer.ticket;
        let file_name = transfer.file_name.clone();
        let view = cx.entity().downgrade();
        cx.spawn(async move |_, cx| {
            let result = task.await;
            let _ = view.update(cx, |this, cx| {
                let message = match result {
                    Ok(()) => format!(
                        "{} incoming transfer '{}'.",
                        if accept { "Accepted" } else { "Rejected" },
                        file_name
                    ),
                    Err(error) => format!(
                        "Couldn't {} incoming transfer '{}': {error}",
                        if accept { "accept" } else { "reject" },
                        file_name
                    ),
                };
                this.transfers.clear_ticket_pending(ticket, message);
                cx.notify();
            });
            Ok::<_, anyhow::Error>(())
        })
        .detach();
    }

    pub(super) fn request_transfer_cancel(
        &mut self,
        transfer: &crate::workspace::view_state::ActiveTransferViewState,
        cx: &mut Context<Self>,
    ) {
        let Some(task) =
            transfer_actions::cancel_transfer_task(&self.controller, transfer.ticket, cx)
        else {
            self.transfers.clear_ticket_pending(
                transfer.ticket,
                "Transfers are still loading.".to_string(),
            );
            cx.notify();
            return;
        };

        self.transfers.mark_ticket_pending(
            transfer.ticket,
            format!("Cancelling transfer '{}'.", transfer.file_name),
        );
        cx.notify();

        let ticket = transfer.ticket;
        let file_name = transfer.file_name.clone();
        let view = cx.entity().downgrade();
        cx.spawn(async move |_, cx| {
            let result = task.await;
            let _ = view.update(cx, |this, cx| {
                let message = match result {
                    Ok(()) => format!("Cancelled transfer '{}'.", file_name),
                    Err(error) => format!("Couldn't cancel transfer '{}': {error}", file_name),
                };
                this.transfers.clear_ticket_pending(ticket, message);
                cx.notify();
            });
            Ok::<_, anyhow::Error>(())
        })
        .detach();
    }
}
