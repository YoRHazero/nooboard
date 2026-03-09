mod support;

use nooboard_app::{
    AppError, AppEvent, DesktopAppService, IncomingTransferDecision, IncomingTransferDisposition,
    NoobId, SendFileItem, SendFilesRequest, TransferId, TransferOutcome, TransferState,
};
use tokio::time::Duration;

use support::{
    TestError, connect_service_pair, new_service_pair, wait_for_event, wait_for_service_state,
};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn send_files_returns_authoritative_transfer_id_in_active_state() -> Result<(), TestError> {
    let (env_a, env_b) = new_service_pair()?;
    let service_a = &env_a.service;
    let service_b = &env_b.service;
    let (noob_id_a, noob_id_b) = connect_service_pair(service_a, service_b).await?;

    let source_file = env_a.dir.path().join("authoritative.txt");
    std::fs::write(&source_file, b"authoritative-transfer-id")?;

    let transfer_ids = service_a
        .send_files(SendFilesRequest {
            targets: vec![noob_id_b.clone()],
            files: vec![SendFileItem {
                path: source_file.clone(),
            }],
        })
        .await?;
    assert_eq!(transfer_ids.len(), 1);
    let transfer_id = transfer_ids[0].clone();
    assert_eq!(transfer_id.peer_noob_id(), &noob_id_b);
    assert!(transfer_id.raw_id() > 0);

    let sender_state = service_a.get_state().await?;
    let active = sender_state
        .transfers
        .active
        .iter()
        .find(|transfer| transfer.transfer_id == transfer_id)
        .expect("returned transfer id must exist in active state");
    assert_eq!(active.peer_noob_id, noob_id_b);
    assert_eq!(active.file_name, "authoritative.txt");

    let receiver_state = wait_for_service_state(service_b, Duration::from_secs(10), |state| {
        state
            .transfers
            .incoming_pending
            .iter()
            .any(|pending| pending.peer_noob_id == noob_id_a)
    })
    .await?;
    let incoming = receiver_state
        .transfers
        .incoming_pending
        .iter()
        .find(|pending| pending.peer_noob_id == noob_id_a)
        .expect("receiver must expose incoming pending transfer");
    assert_eq!(incoming.transfer_id.raw_id(), transfer_id.raw_id());

    service_a.cancel_transfer(transfer_id).await?;
    service_a.shutdown().await?;
    service_b.shutdown().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn decide_incoming_transfer_accept_moves_to_succeeded_completion() -> Result<(), TestError> {
    let (env_a, env_b) = new_service_pair()?;
    let service_a = &env_a.service;
    let service_b = &env_b.service;
    let (noob_id_a, noob_id_b) = connect_service_pair(service_a, service_b).await?;
    let mut sender_events = service_a.subscribe_events().await?;
    let mut receiver_events = service_b.subscribe_events().await?;

    let source_file = env_a.dir.path().join("accept-me.txt");
    std::fs::write(&source_file, b"accepted-content")?;

    let sender_transfer_id = service_a
        .send_files(SendFilesRequest {
            targets: vec![noob_id_b.clone()],
            files: vec![SendFileItem { path: source_file }],
        })
        .await?
        .into_iter()
        .next()
        .expect("must create transfer");

    let receiver_pending = wait_for_service_state(service_b, Duration::from_secs(10), |state| {
        state
            .transfers
            .incoming_pending
            .iter()
            .any(|pending| pending.peer_noob_id == noob_id_a)
    })
    .await?;
    let receiver_transfer_id = receiver_pending
        .transfers
        .incoming_pending
        .iter()
        .find(|pending| pending.peer_noob_id == noob_id_a)
        .expect("receiver must expose pending transfer")
        .transfer_id
        .clone();
    assert_eq!(receiver_transfer_id.raw_id(), sender_transfer_id.raw_id());

    service_b
        .decide_incoming_transfer(IncomingTransferDecision {
            transfer_id: receiver_transfer_id.clone(),
            decision: IncomingTransferDisposition::Accept,
        })
        .await?;

    let receiver_after_accept = service_b.get_state().await?;
    assert!(
        !receiver_after_accept
            .transfers
            .incoming_pending
            .iter()
            .any(|pending| pending.transfer_id == receiver_transfer_id)
    );

    let receiver_completed_id = wait_for_event(
        &mut receiver_events,
        Duration::from_secs(10),
        |event| match event {
            AppEvent::TransferCompleted {
                transfer_id,
                outcome: TransferOutcome::Succeeded,
            } if transfer_id == receiver_transfer_id => Some(transfer_id),
            _ => None,
        },
    )
    .await?;
    assert_eq!(receiver_completed_id, receiver_transfer_id);
    let receiver_state_at_event = service_b.get_state().await?;
    assert!(
        receiver_state_at_event
            .transfers
            .recent_completed
            .iter()
            .any(|completed| completed.transfer_id == receiver_transfer_id
                && completed.outcome == TransferOutcome::Succeeded
                && completed.saved_path.is_some())
    );

    let sender_completed_id = wait_for_event(
        &mut sender_events,
        Duration::from_secs(10),
        |event| match event {
            AppEvent::TransferCompleted {
                transfer_id,
                outcome: TransferOutcome::Succeeded,
            } if transfer_id == sender_transfer_id => Some(transfer_id),
            _ => None,
        },
    )
    .await?;
    assert_eq!(sender_completed_id, sender_transfer_id);
    let sender_state_at_event = service_a.get_state().await?;
    assert!(
        sender_state_at_event
            .transfers
            .recent_completed
            .iter()
            .any(|completed| completed.transfer_id == sender_transfer_id
                && completed.outcome == TransferOutcome::Succeeded)
    );

    let sender_state = wait_for_service_state(service_a, Duration::from_secs(10), |state| {
        state.transfers.recent_completed.iter().any(|completed| {
            completed.transfer_id == sender_transfer_id
                && completed.outcome == TransferOutcome::Succeeded
        })
    })
    .await?;
    let sender_completed = sender_state
        .transfers
        .recent_completed
        .iter()
        .find(|completed| completed.transfer_id == sender_transfer_id)
        .expect("sender must expose completed transfer");
    assert_eq!(sender_completed.outcome, TransferOutcome::Succeeded);
    assert_eq!(sender_completed.saved_path, None);

    let receiver_state = wait_for_service_state(service_b, Duration::from_secs(10), |state| {
        state.transfers.recent_completed.iter().any(|completed| {
            completed.transfer_id == receiver_transfer_id
                && completed.outcome == TransferOutcome::Succeeded
                && completed.saved_path.is_some()
        })
    })
    .await?;
    let receiver_completed = receiver_state
        .transfers
        .recent_completed
        .iter()
        .find(|completed| completed.transfer_id == receiver_transfer_id)
        .expect("receiver must expose completed transfer");
    let saved_path = receiver_completed
        .saved_path
        .as_ref()
        .expect("downloaded file must have saved_path");
    assert_eq!(std::fs::read_to_string(saved_path)?, "accepted-content");

    service_a.shutdown().await?;
    service_b.shutdown().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn decide_incoming_transfer_reject_moves_both_sides_to_rejected_completion()
-> Result<(), TestError> {
    let (env_a, env_b) = new_service_pair()?;
    let service_a = &env_a.service;
    let service_b = &env_b.service;
    let (noob_id_a, noob_id_b) = connect_service_pair(service_a, service_b).await?;
    let mut sender_events = service_a.subscribe_events().await?;
    let mut receiver_events = service_b.subscribe_events().await?;

    let source_file = env_a.dir.path().join("reject-me.txt");
    std::fs::write(&source_file, b"reject-content")?;

    let sender_transfer_id = service_a
        .send_files(SendFilesRequest {
            targets: vec![noob_id_b.clone()],
            files: vec![SendFileItem { path: source_file }],
        })
        .await?
        .into_iter()
        .next()
        .expect("must create transfer");

    let receiver_pending = wait_for_service_state(service_b, Duration::from_secs(10), |state| {
        state
            .transfers
            .incoming_pending
            .iter()
            .any(|pending| pending.peer_noob_id == noob_id_a)
    })
    .await?;
    let receiver_transfer_id = receiver_pending
        .transfers
        .incoming_pending
        .iter()
        .find(|pending| pending.peer_noob_id == noob_id_a)
        .expect("receiver must expose pending transfer")
        .transfer_id
        .clone();
    assert_eq!(receiver_transfer_id.raw_id(), sender_transfer_id.raw_id());

    service_b
        .decide_incoming_transfer(IncomingTransferDecision {
            transfer_id: receiver_transfer_id.clone(),
            decision: IncomingTransferDisposition::Reject,
        })
        .await?;

    let receiver_completed_id = wait_for_event(
        &mut receiver_events,
        Duration::from_secs(10),
        |event| match event {
            AppEvent::TransferCompleted {
                transfer_id,
                outcome: TransferOutcome::Rejected,
            } if transfer_id == receiver_transfer_id => Some(transfer_id),
            _ => None,
        },
    )
    .await?;
    assert_eq!(receiver_completed_id, receiver_transfer_id);
    let receiver_state_at_event = service_b.get_state().await?;
    assert!(
        receiver_state_at_event
            .transfers
            .recent_completed
            .iter()
            .any(|completed| completed.transfer_id == receiver_transfer_id
                && completed.outcome == TransferOutcome::Rejected)
    );

    let sender_completed_id = wait_for_event(
        &mut sender_events,
        Duration::from_secs(10),
        |event| match event {
            AppEvent::TransferCompleted {
                transfer_id,
                outcome: TransferOutcome::Rejected,
            } if transfer_id == sender_transfer_id => Some(transfer_id),
            _ => None,
        },
    )
    .await?;
    assert_eq!(sender_completed_id, sender_transfer_id);
    let sender_state_at_event = service_a.get_state().await?;
    assert!(
        sender_state_at_event
            .transfers
            .recent_completed
            .iter()
            .any(|completed| completed.transfer_id == sender_transfer_id
                && completed.outcome == TransferOutcome::Rejected)
    );

    let receiver_state = wait_for_service_state(service_b, Duration::from_secs(10), |state| {
        !state
            .transfers
            .incoming_pending
            .iter()
            .any(|pending| pending.transfer_id == receiver_transfer_id)
            && state.transfers.recent_completed.len() == 1
            && state.transfers.recent_completed.iter().any(|completed| {
                completed.transfer_id == receiver_transfer_id
                    && completed.outcome == TransferOutcome::Rejected
            })
    })
    .await?;
    assert_eq!(receiver_state.transfers.recent_completed.len(), 1);

    let sender_state = wait_for_service_state(service_a, Duration::from_secs(10), |state| {
        state.transfers.recent_completed.iter().any(|completed| {
            completed.transfer_id == sender_transfer_id
                && completed.outcome == TransferOutcome::Rejected
        })
    })
    .await?;
    assert!(
        sender_state
            .transfers
            .recent_completed
            .iter()
            .any(|completed| completed.transfer_id == sender_transfer_id
                && completed.outcome == TransferOutcome::Rejected)
    );

    service_a.shutdown().await?;
    service_b.shutdown().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancel_transfer_moves_transfer_to_cancelled_completion() -> Result<(), TestError> {
    let (env_a, env_b) = new_service_pair()?;
    let service_a = &env_a.service;
    let service_b = &env_b.service;
    let (noob_id_a, noob_id_b) = connect_service_pair(service_a, service_b).await?;

    let source_file = env_a.dir.path().join("cancel-me.txt");
    std::fs::write(&source_file, vec![b'x'; 256 * 1024])?;

    let transfer_id = service_a
        .send_files(SendFilesRequest {
            targets: vec![noob_id_b.clone()],
            files: vec![SendFileItem { path: source_file }],
        })
        .await?
        .into_iter()
        .next()
        .expect("must create transfer");

    wait_for_service_state(service_b, Duration::from_secs(10), |state| {
        state
            .transfers
            .incoming_pending
            .iter()
            .any(|pending| pending.peer_noob_id == noob_id_a)
    })
    .await?;

    let mut sender_events = service_a.subscribe_events().await?;
    service_a.cancel_transfer(transfer_id.clone()).await?;

    let updated_transfer_id = wait_for_event(
        &mut sender_events,
        Duration::from_secs(10),
        |event| match event {
            AppEvent::TransferUpdated {
                transfer_id: updated_transfer_id,
            } if updated_transfer_id == transfer_id => Some(updated_transfer_id),
            _ => None,
        },
    )
    .await?;
    assert_eq!(updated_transfer_id, transfer_id);

    let sender_snapshot = service_a.get_state().await?;
    if let Some(active) = sender_snapshot
        .transfers
        .active
        .iter()
        .find(|transfer| transfer.transfer_id == transfer_id)
    {
        assert_eq!(active.state, TransferState::Cancelling);
    } else {
        assert!(
            sender_snapshot
                .transfers
                .recent_completed
                .iter()
                .any(|completed| completed.transfer_id == transfer_id
                    && completed.outcome == TransferOutcome::Cancelled)
        );
    }

    let completed_transfer_id = wait_for_event(
        &mut sender_events,
        Duration::from_secs(10),
        |event| match event {
            AppEvent::TransferCompleted {
                transfer_id: completed_transfer_id,
                outcome: TransferOutcome::Cancelled,
            } if completed_transfer_id == transfer_id => Some(completed_transfer_id),
            _ => None,
        },
    )
    .await?;
    assert_eq!(completed_transfer_id, transfer_id);
    let sender_state_at_event = service_a.get_state().await?;
    assert!(
        sender_state_at_event
            .transfers
            .recent_completed
            .iter()
            .any(|completed| completed.transfer_id == transfer_id
                && completed.outcome == TransferOutcome::Cancelled)
    );

    let sender_state = wait_for_service_state(service_a, Duration::from_secs(10), |state| {
        !state
            .transfers
            .active
            .iter()
            .any(|transfer| transfer.transfer_id == transfer_id)
            && state.transfers.recent_completed.iter().any(|completed| {
                completed.transfer_id == transfer_id
                    && completed.outcome == TransferOutcome::Cancelled
            })
    })
    .await?;
    assert!(
        sender_state
            .transfers
            .recent_completed
            .iter()
            .any(|completed| completed.transfer_id == transfer_id
                && completed.outcome == TransferOutcome::Cancelled)
    );

    let receiver_state = wait_for_service_state(service_b, Duration::from_secs(10), |state| {
        state.transfers.incoming_pending.is_empty()
            && state.transfers.recent_completed.iter().any(|completed| {
                completed.transfer_id.peer_noob_id() == &noob_id_a
                    && completed.transfer_id.raw_id() == transfer_id.raw_id()
                    && completed.outcome == TransferOutcome::Cancelled
            })
    })
    .await?;
    assert!(
        receiver_state
            .transfers
            .recent_completed
            .iter()
            .any(|completed| {
                completed.transfer_id.peer_noob_id() == &noob_id_a
                    && completed.transfer_id.raw_id() == transfer_id.raw_id()
                    && completed.outcome == TransferOutcome::Cancelled
            })
    );

    service_a.shutdown().await?;
    service_b.shutdown().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn transfer_operations_return_structured_errors_for_invalid_states() -> Result<(), TestError>
{
    let (env_a, env_b) = new_service_pair()?;
    let service_a = &env_a.service;
    let service_b = &env_b.service;
    let (noob_id_a, noob_id_b) = connect_service_pair(service_a, service_b).await?;

    let missing_transfer_id = TransferId::new(NoobId::new("missing-peer"), 999);
    let decide_missing_error = service_a
        .decide_incoming_transfer(IncomingTransferDecision {
            transfer_id: missing_transfer_id.clone(),
            decision: IncomingTransferDisposition::Accept,
        })
        .await
        .expect_err("missing pending transfer must fail");
    assert!(matches!(
        decide_missing_error,
        AppError::TransferNotFound { .. }
    ));

    let cancel_missing_error = service_a
        .cancel_transfer(missing_transfer_id)
        .await
        .expect_err("missing transfer must fail");
    assert!(matches!(
        cancel_missing_error,
        AppError::TransferNotFound { .. }
    ));

    let source_file = env_a.dir.path().join("invalid-states.txt");
    std::fs::write(&source_file, b"invalid-states")?;

    let sender_transfer_id = service_a
        .send_files(SendFilesRequest {
            targets: vec![noob_id_b.clone()],
            files: vec![SendFileItem { path: source_file }],
        })
        .await?
        .into_iter()
        .next()
        .expect("must create transfer");

    let receiver_pending = wait_for_service_state(service_b, Duration::from_secs(10), |state| {
        state
            .transfers
            .incoming_pending
            .iter()
            .any(|pending| pending.peer_noob_id == noob_id_a)
    })
    .await?;
    let receiver_transfer_id = receiver_pending
        .transfers
        .incoming_pending
        .iter()
        .find(|pending| pending.peer_noob_id == noob_id_a)
        .expect("receiver must expose pending transfer")
        .transfer_id
        .clone();

    let pending_cancel_error = service_b
        .cancel_transfer(receiver_transfer_id.clone())
        .await
        .expect_err("pending incoming transfer must not be cancellable via cancel_transfer");
    assert!(matches!(
        pending_cancel_error,
        AppError::TransferNotCancelable { .. }
    ));

    service_b
        .decide_incoming_transfer(IncomingTransferDecision {
            transfer_id: receiver_transfer_id,
            decision: IncomingTransferDisposition::Reject,
        })
        .await?;

    wait_for_service_state(service_a, Duration::from_secs(10), |state| {
        state.transfers.recent_completed.iter().any(|completed| {
            completed.transfer_id == sender_transfer_id
                && completed.outcome == TransferOutcome::Rejected
        })
    })
    .await?;

    let completed_cancel_error = service_a
        .cancel_transfer(sender_transfer_id)
        .await
        .expect_err("completed transfer must not be cancellable");
    assert!(matches!(
        completed_cancel_error,
        AppError::TransferNotCancelable { .. }
    ));

    service_a.shutdown().await?;
    service_b.shutdown().await?;
    Ok(())
}
