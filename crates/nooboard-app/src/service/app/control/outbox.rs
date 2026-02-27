use std::time::Duration;

use nooboard_storage::OutboxMessage;
use nooboard_sync::SendTextRequest;
use tokio::sync::{mpsc, watch};
use tokio::time::{MissedTickBehavior, interval, timeout};
use uuid::Uuid;

use crate::service::types::{SyncDesiredState, now_millis_i64};
use crate::{AppError, AppResult};

use super::command::ControlCommand;
use super::state::{ControlState, OutboxTickerHandle};

const OUTBOX_POLL_INTERVAL: Duration = Duration::from_millis(250);
const OUTBOX_STOP_TIMEOUT: Duration = Duration::from_secs(2);
const OUTBOX_BATCH_SIZE: usize = 32;
const OUTBOX_LEASE_MS: i64 = 5_000;
const OUTBOX_ENGINE_DOWN_RETRY_MS: i64 = 1_000;
const OUTBOX_RETRY_BASE_MS: i64 = 500;
const OUTBOX_RETRY_MAX_MS: i64 = 30_000;

pub(super) fn start_outbox_ticker(command_tx: mpsc::Sender<ControlCommand>) -> OutboxTickerHandle {
    let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
    let task = tokio::spawn(async move {
        let mut ticker = interval(OUTBOX_POLL_INTERVAL);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                changed = shutdown_rx.changed() => {
                    match changed {
                        Ok(()) if *shutdown_rx.borrow() => break,
                        Ok(()) => continue,
                        Err(_) => break,
                    }
                }
                _ = ticker.tick() => {}
            }

            if *shutdown_rx.borrow() {
                break;
            }

            match command_tx.try_send(ControlCommand::TickOutbox) {
                Ok(()) => {}
                Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {}
                Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => break,
            }
        }
    });

    OutboxTickerHandle { task, shutdown_tx }
}

pub(super) async fn stop_outbox_ticker(state: &mut ControlState) {
    let Some(handle) = state.outbox_ticker.take() else {
        return;
    };

    let _ = handle.shutdown_tx.send(true);
    let mut task = handle.task;
    if timeout(OUTBOX_STOP_TIMEOUT, &mut task).await.is_err() {
        task.abort();
        let _ = task.await;
    }
}

pub(super) async fn tick_outbox(state: &mut ControlState) {
    if state.desired_state != SyncDesiredState::Running {
        return;
    }
    if !state.config.sync.network.enabled {
        return;
    }

    let _ = dispatch_due_outbox_once(state).await;
}

async fn dispatch_due_outbox_once(state: &mut ControlState) -> AppResult<()> {
    let now_ms = now_millis_i64();
    let due = state
        .storage_runtime
        .list_due_outbox(now_ms, OUTBOX_BATCH_SIZE)
        .await?;

    for message in due {
        let now_ms = now_millis_i64();
        let lease_until_ms = now_ms.saturating_add(OUTBOX_LEASE_MS);
        let leased = state
            .storage_runtime
            .try_lease_outbox_message(message.id, lease_until_ms, now_ms)
            .await?;
        if !leased {
            continue;
        }

        match dispatch_one_outbox_message(&state.sync_runtime, &message).await {
            Ok(()) => {
                let _ = state
                    .storage_runtime
                    .mark_outbox_sent(message.id, now_millis_i64())
                    .await?;
            }
            Err(error) => {
                let delay_ms = retry_delay_ms(message.attempt_count, error.engine_unavailable);
                let next_attempt_at_ms = now_ms.saturating_add(delay_ms);
                let _ = state
                    .storage_runtime
                    .mark_outbox_retry(message.id, next_attempt_at_ms, error.message, now_ms)
                    .await?;
            }
        }
    }

    Ok(())
}

struct DispatchFailure {
    message: String,
    engine_unavailable: bool,
}

async fn dispatch_one_outbox_message(
    sync_runtime: &crate::sync_runtime::SyncRuntime,
    message: &OutboxMessage,
) -> Result<(), DispatchFailure> {
    let text_tx = match sync_runtime.text_sender() {
        Ok(tx) => tx,
        Err(AppError::EngineNotRunning) => {
            return Err(DispatchFailure {
                message: "sync engine is not running".to_string(),
                engine_unavailable: true,
            });
        }
        Err(error) => {
            return Err(DispatchFailure {
                message: error.to_string(),
                engine_unavailable: false,
            });
        }
    };

    let connected_peer_ids = sync_runtime
        .connected_peers()
        .into_iter()
        .map(|peer| peer.peer_node_id)
        .collect::<Vec<_>>();

    if !has_eligible_peer(&connected_peer_ids, message.targets.as_deref()) {
        return Err(DispatchFailure {
            message: "no eligible connected peers for outbox message".to_string(),
            engine_unavailable: true,
        });
    }

    let request = SendTextRequest {
        event_id: Uuid::from_bytes(message.event_id).to_string(),
        content: message.content.clone(),
        targets: message.targets.clone(),
    };

    text_tx
        .send(request)
        .await
        .map_err(|error| DispatchFailure {
            message: format!("sync text_tx closed: {error}"),
            engine_unavailable: true,
        })
}

fn retry_delay_ms(attempt_count: u32, engine_unavailable: bool) -> i64 {
    if engine_unavailable {
        return OUTBOX_ENGINE_DOWN_RETRY_MS;
    }

    let exp = attempt_count.min(8);
    let raw = OUTBOX_RETRY_BASE_MS.saturating_mul(1_i64 << exp);
    raw.clamp(OUTBOX_RETRY_BASE_MS, OUTBOX_RETRY_MAX_MS)
}

fn has_eligible_peer(connected_peer_ids: &[String], targets: Option<&[String]>) -> bool {
    if connected_peer_ids.is_empty() {
        return false;
    }

    match targets {
        None => true,
        Some(targets) => targets.iter().any(|target| {
            connected_peer_ids
                .iter()
                .any(|peer_id| peer_id == target.trim())
        }),
    }
}
