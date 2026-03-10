use std::collections::HashSet;

use nooboard_sync::{
    CancelTransferRequest as SyncCancelTransferRequest, FileDecisionInput,
    SendFileRequest as SyncSendFileRequest,
};

use crate::service::mappers::map_transfer_id;
use crate::service::types::{
    AppEvent, CompletedTransfer, IncomingTransfer, IncomingTransferDecision,
    IncomingTransferDisposition, NoobId, SendFilesRequest, Transfer, TransferDirection, TransferId,
    TransferOutcome, TransferState, now_millis_i64,
};
use crate::{AppError, AppResult};

use super::state::ControlState;

pub(super) async fn send_files(
    state: &mut ControlState,
    request: SendFilesRequest,
) -> AppResult<Vec<TransferId>> {
    if !state.config.sync.network.enabled {
        return Err(AppError::SyncDisabled);
    }

    let targets = dedup_connected_targets(state, &request.targets)?;
    let mut created = Vec::new();

    let target_names: Vec<String> = targets
        .iter()
        .map(|target| target.as_str().to_string())
        .collect();

    for file in &request.files {
        let metadata = std::fs::metadata(&file.path)?;
        let file_name = file
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| {
                AppError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("invalid file name: {}", file.path.display()),
                ))
            })?
            .to_string();

        let scheduled = state
            .sync_runtime
            .send_file(SyncSendFileRequest {
                path: file.path.clone(),
                targets: Some(target_names.clone()),
            })
            .await?;

        let now_ms = now_millis_i64();
        let mut new_transfer_ids = Vec::with_capacity(scheduled.len());
        let scheduled_peer_device_ids = scheduled
            .iter()
            .map(|scheduled_transfer| {
                let peer_noob_id = NoobId::new(scheduled_transfer.peer_noob_id.clone());
                (peer_noob_id.clone(), state.peer_device_id(&peer_noob_id))
            })
            .collect::<Vec<_>>();
        state.update_state(|app_state| {
            for (scheduled_transfer, (peer_noob_id, peer_device_id)) in
                scheduled.iter().zip(scheduled_peer_device_ids.iter())
            {
                let transfer_id =
                    TransferId::new(peer_noob_id.clone(), scheduled_transfer.transfer_id);
                new_transfer_ids.push(transfer_id.clone());
                app_state.transfers.active.push(Transfer {
                    transfer_id,
                    direction: TransferDirection::Upload,
                    peer_device_id: peer_device_id.clone(),
                    peer_noob_id: peer_noob_id.clone(),
                    file_name: file_name.clone(),
                    file_size: metadata.len(),
                    transferred_bytes: 0,
                    state: TransferState::Queued,
                    started_at_ms: now_ms,
                    updated_at_ms: now_ms,
                });
            }
        });
        created.extend(new_transfer_ids);
    }

    Ok(created)
}

pub(super) async fn decide_incoming_transfer(
    state: &mut ControlState,
    request: IncomingTransferDecision,
) -> AppResult<()> {
    if !state.config.sync.network.enabled {
        return Err(AppError::SyncDisabled);
    }

    let Some(pending) = state
        .app_state
        .transfers
        .incoming_pending
        .iter()
        .find(|pending| pending.transfer_id == request.transfer_id)
        .cloned()
    else {
        return Err(AppError::TransferNotFound {
            transfer_id: request.transfer_id.to_string(),
        });
    };

    let decision_tx = state.sync_runtime.decision_sender()?;
    decision_tx
        .send(FileDecisionInput {
            peer_noob_id: pending.peer_noob_id.as_str().to_string(),
            transfer_id: request.transfer_id.raw_id(),
            accept: matches!(request.decision, IncomingTransferDisposition::Accept),
            reason: None,
        })
        .await
        .map_err(|error| AppError::ChannelClosed(format!("sync decision_tx closed: {error}")))?;

    match request.decision {
        IncomingTransferDisposition::Accept => {
            let accepted_at_ms = now_millis_i64();
            let transfer_id_for_state = request.transfer_id.clone();
            state.update_state(|app_state| {
                app_state
                    .transfers
                    .incoming_pending
                    .retain(|item| item.transfer_id != request.transfer_id);
                if !app_state
                    .transfers
                    .active
                    .iter()
                    .any(|item| item.transfer_id == transfer_id_for_state)
                {
                    app_state.transfers.active.push(Transfer {
                        transfer_id: transfer_id_for_state.clone(),
                        direction: TransferDirection::Download,
                        peer_device_id: pending.peer_device_id.clone(),
                        peer_noob_id: pending.peer_noob_id.clone(),
                        file_name: pending.file_name.clone(),
                        file_size: pending.file_size,
                        transferred_bytes: 0,
                        state: TransferState::Starting,
                        started_at_ms: accepted_at_ms,
                        updated_at_ms: accepted_at_ms,
                    });
                }
            });
            state.publish_event(AppEvent::TransferUpdated {
                transfer_id: request.transfer_id,
            });
        }
        IncomingTransferDisposition::Reject => {
            let finished_at_ms = now_millis_i64();
            let completed = CompletedTransfer {
                transfer_id: request.transfer_id.clone(),
                direction: TransferDirection::Download,
                peer_device_id: pending.peer_device_id.clone(),
                peer_noob_id: pending.peer_noob_id,
                file_name: pending.file_name,
                file_size: pending.file_size,
                outcome: TransferOutcome::Rejected,
                started_at_ms: None,
                finished_at_ms,
                saved_path: None,
                message: None,
            };
            complete_transfer(state, request.transfer_id, completed);
        }
    }

    Ok(())
}

pub(super) async fn cancel_transfer(
    state: &mut ControlState,
    transfer_id: TransferId,
) -> AppResult<()> {
    let Some(existing) = state
        .app_state
        .transfers
        .active
        .iter()
        .find(|transfer| transfer.transfer_id == transfer_id)
        .cloned()
    else {
        if state
            .app_state
            .transfers
            .incoming_pending
            .iter()
            .any(|transfer| transfer.transfer_id == transfer_id)
            || state
                .app_state
                .transfers
                .recent_completed
                .iter()
                .any(|transfer| transfer.transfer_id == transfer_id)
        {
            return Err(AppError::TransferNotCancelable {
                transfer_id: transfer_id.to_string(),
            });
        }
        return Err(AppError::TransferNotFound {
            transfer_id: transfer_id.to_string(),
        });
    };

    if existing.state == TransferState::Cancelling {
        return Err(AppError::TransferNotCancelable {
            transfer_id: transfer_id.to_string(),
        });
    }

    state
        .sync_runtime
        .cancel_transfer(SyncCancelTransferRequest {
            peer_noob_id: transfer_id.peer_noob_id().as_str().to_string(),
            transfer_id: transfer_id.raw_id(),
        })
        .await?;

    let transfer_id_for_state = transfer_id.clone();
    state.update_state(|app_state| {
        if let Some(active) = app_state
            .transfers
            .active
            .iter_mut()
            .find(|transfer| transfer.transfer_id == transfer_id_for_state)
        {
            active.state = TransferState::Cancelling;
            active.updated_at_ms = now_millis_i64();
        }
    });
    if existing.state != TransferState::Cancelling {
        state.publish_event(AppEvent::TransferUpdated { transfer_id });
    }

    Ok(())
}

pub(super) fn handle_incoming_offer(
    state: &mut ControlState,
    peer_noob_id: crate::service::types::NoobId,
    raw_transfer_id: u32,
    file_name: String,
    file_size: u64,
    total_chunks: u32,
) {
    let transfer_id = TransferId::new(peer_noob_id.clone(), raw_transfer_id);
    let peer_device_id = state.peer_device_id(&peer_noob_id);
    let already_pending = state
        .app_state
        .transfers
        .incoming_pending
        .iter()
        .any(|pending| pending.transfer_id == transfer_id);

    let transfer_id_for_state = transfer_id.clone();
    state.update_state(|app_state| {
        // If transfer updates raced ahead of the decision request, pending still wins:
        // pre-accept incoming transfers must not appear in active.
        app_state
            .transfers
            .active
            .retain(|item| item.transfer_id != transfer_id_for_state);
        if !already_pending {
            app_state.transfers.incoming_pending.push(IncomingTransfer {
                transfer_id: transfer_id_for_state,
                peer_noob_id: peer_noob_id.clone(),
                peer_device_id: peer_device_id.clone(),
                file_name,
                file_size,
                total_chunks,
                offered_at_ms: now_millis_i64(),
            });
        }
    });
    if !already_pending {
        state.publish_event(AppEvent::IncomingTransferOffered { transfer_id });
    }
}

pub(super) fn apply_transfer_update(
    state: &mut ControlState,
    update: nooboard_sync::TransferUpdate,
) {
    let transfer_id = map_transfer_id(&update);
    let direction: TransferDirection = update.direction.clone().into();
    let peer_noob_id = transfer_id.peer_noob_id().clone();
    let now_ms = now_millis_i64();

    match &update.state {
        nooboard_sync::TransferState::Started {
            file_name,
            total_bytes,
        } => {
            let transfer_id_for_state = transfer_id.clone();
            let peer_for_state = peer_noob_id.clone();
            let peer_device_id = state.peer_device_id(&peer_for_state);
            let file_name_for_state = file_name.clone();
            state.update_state(|app_state| {
                if let Some(existing) = app_state
                    .transfers
                    .active
                    .iter_mut()
                    .find(|transfer| transfer.transfer_id == transfer_id_for_state)
                {
                    existing.file_name = file_name_for_state.clone();
                    existing.file_size = *total_bytes;
                    existing.state = TransferState::Starting;
                    existing.updated_at_ms = now_ms;
                } else if !(direction == TransferDirection::Download
                    && app_state
                        .transfers
                        .incoming_pending
                        .iter()
                        .any(|transfer| transfer.transfer_id == transfer_id_for_state))
                {
                    app_state.transfers.active.push(Transfer {
                        transfer_id: transfer_id_for_state.clone(),
                        direction,
                        peer_device_id: peer_device_id.clone(),
                        peer_noob_id: peer_for_state.clone(),
                        file_name: file_name_for_state.clone(),
                        file_size: *total_bytes,
                        transferred_bytes: 0,
                        state: TransferState::Starting,
                        started_at_ms: now_ms,
                        updated_at_ms: now_ms,
                    });
                }
            });
            state.publish_event(AppEvent::TransferUpdated { transfer_id });
        }
        nooboard_sync::TransferState::Progress {
            done_bytes,
            total_bytes,
            ..
        } => {
            let transfer_id_for_state = transfer_id.clone();
            let peer_for_state = peer_noob_id.clone();
            let peer_device_id = state.peer_device_id(&peer_for_state);
            state.update_state(|app_state| {
                if let Some(existing) = app_state
                    .transfers
                    .active
                    .iter_mut()
                    .find(|transfer| transfer.transfer_id == transfer_id_for_state)
                {
                    existing.file_size = *total_bytes;
                    existing.transferred_bytes = *done_bytes;
                    existing.state = TransferState::InProgress;
                    existing.updated_at_ms = now_ms;
                } else if !(direction == TransferDirection::Download
                    && app_state
                        .transfers
                        .incoming_pending
                        .iter()
                        .any(|transfer| transfer.transfer_id == transfer_id_for_state))
                {
                    app_state.transfers.active.push(Transfer {
                        transfer_id: transfer_id_for_state.clone(),
                        direction,
                        peer_device_id: peer_device_id.clone(),
                        peer_noob_id: peer_for_state.clone(),
                        file_name: String::new(),
                        file_size: *total_bytes,
                        transferred_bytes: *done_bytes,
                        state: TransferState::InProgress,
                        started_at_ms: now_ms,
                        updated_at_ms: now_ms,
                    });
                }
            });
            state.publish_event(AppEvent::TransferUpdated { transfer_id });
        }
        nooboard_sync::TransferState::Finished { path } => {
            let completed = take_completed_from_active(
                state,
                &transfer_id,
                direction,
                peer_noob_id,
                TransferOutcome::Succeeded,
                now_ms,
                path.clone(),
                None,
            );
            complete_transfer(state, transfer_id, completed);
        }
        nooboard_sync::TransferState::Failed { reason } => {
            let completed = take_completed_from_active(
                state,
                &transfer_id,
                direction,
                peer_noob_id,
                TransferOutcome::Failed,
                now_ms,
                None,
                Some(reason.clone()),
            );
            complete_transfer(state, transfer_id, completed);
        }
        nooboard_sync::TransferState::Rejected { reason } => {
            let completed = take_completed_from_active(
                state,
                &transfer_id,
                direction,
                peer_noob_id,
                TransferOutcome::Rejected,
                now_ms,
                None,
                reason.clone(),
            );
            complete_transfer(state, transfer_id, completed);
        }
        nooboard_sync::TransferState::Cancelled { reason } => {
            let completed = take_completed_from_active(
                state,
                &transfer_id,
                direction,
                peer_noob_id,
                TransferOutcome::Cancelled,
                now_ms,
                None,
                reason.clone(),
            );
            complete_transfer(state, transfer_id, completed);
        }
    }
}

fn dedup_connected_targets(
    state: &ControlState,
    targets: &[crate::service::types::NoobId],
) -> AppResult<Vec<crate::service::types::NoobId>> {
    let connected: HashSet<&str> = state
        .app_state
        .peers
        .connected
        .iter()
        .map(|peer| peer.noob_id.as_str())
        .collect();
    let mut deduped = Vec::new();
    let mut seen = HashSet::new();
    for target in targets {
        if !connected.contains(target.as_str()) {
            return Err(AppError::PeerNotConnected {
                peer_noob_id: target.as_str().to_string(),
            });
        }
        if seen.insert(target.as_str().to_string()) {
            deduped.push(target.clone());
        }
    }
    Ok(deduped)
}

fn complete_transfer(
    state: &mut ControlState,
    transfer_id: TransferId,
    completed: CompletedTransfer,
) {
    let transfer_id_for_state = transfer_id.clone();
    let recent_completed_limit = state.recent_completed_limit();
    state.update_state(|app_state| {
        app_state
            .transfers
            .incoming_pending
            .retain(|item| item.transfer_id != transfer_id_for_state);
        app_state
            .transfers
            .active
            .retain(|item| item.transfer_id != transfer_id_for_state);
        app_state
            .transfers
            .recent_completed
            .insert(0, completed.clone());
        app_state
            .transfers
            .recent_completed
            .truncate(recent_completed_limit);
    });
    state.publish_event(AppEvent::TransferCompleted {
        transfer_id,
        outcome: completed.outcome,
    });
}

fn take_completed_from_active(
    state: &mut ControlState,
    transfer_id: &TransferId,
    direction: TransferDirection,
    peer_noob_id: crate::service::types::NoobId,
    outcome: TransferOutcome,
    finished_at_ms: i64,
    saved_path: Option<std::path::PathBuf>,
    message: Option<String>,
) -> CompletedTransfer {
    let active = state
        .app_state
        .transfers
        .active
        .iter()
        .find(|transfer| &transfer.transfer_id == transfer_id)
        .cloned();
    CompletedTransfer {
        transfer_id: transfer_id.clone(),
        direction,
        peer_device_id: active
            .as_ref()
            .map(|transfer| transfer.peer_device_id.clone())
            .unwrap_or_else(|| state.peer_device_id(&peer_noob_id)),
        peer_noob_id,
        file_name: active
            .as_ref()
            .map(|transfer| transfer.file_name.clone())
            .unwrap_or_default(),
        file_size: active
            .as_ref()
            .map(|transfer| transfer.file_size)
            .unwrap_or(0),
        outcome,
        started_at_ms: active.as_ref().map(|transfer| transfer.started_at_ms),
        finished_at_ms,
        saved_path,
        message,
    }
}
