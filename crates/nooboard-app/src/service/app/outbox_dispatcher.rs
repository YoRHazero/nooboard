use std::sync::Arc;
use std::time::Duration;

use nooboard_storage::OutboxMessage;
use nooboard_sync::SendTextRequest;
use tokio::sync::{Mutex, RwLock, watch};
use tokio::time::{MissedTickBehavior, interval, timeout};
use uuid::Uuid;

use crate::config::AppConfig;
use crate::service::types::now_millis_i64;
use crate::sync_runtime::SyncRuntime;
use crate::{AppError, AppResult};

use super::{AppServiceImpl, OutboxDispatcherHandle};

const OUTBOX_POLL_INTERVAL: Duration = Duration::from_millis(250);
const OUTBOX_STOP_TIMEOUT: Duration = Duration::from_secs(2);
const OUTBOX_BATCH_SIZE: usize = 32;
const OUTBOX_LEASE_MS: i64 = 5_000;
const OUTBOX_ENGINE_DOWN_RETRY_MS: i64 = 1_000;
const OUTBOX_RETRY_BASE_MS: i64 = 500;
const OUTBOX_RETRY_MAX_MS: i64 = 30_000;

impl AppServiceImpl {
    pub(super) async fn start_outbox_dispatcher_if_needed(&self) -> AppResult<()> {
        let mut guard = self.outbox_dispatcher.lock().await;
        if guard.is_some() {
            return Ok(());
        }

        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let storage_runtime = Arc::clone(&self.storage_runtime);
        let sync_runtime = Arc::clone(&self.sync_runtime);
        let config = Arc::clone(&self.config);

        let task = tokio::spawn(async move {
            run_outbox_dispatcher(storage_runtime, sync_runtime, config, shutdown_rx).await;
        });

        *guard = Some(OutboxDispatcherHandle { task, shutdown_tx });
        Ok(())
    }

    pub(super) async fn stop_outbox_dispatcher(&self) {
        let handle = {
            let mut guard = self.outbox_dispatcher.lock().await;
            guard.take()
        };

        let Some(handle) = handle else {
            return;
        };

        let _ = handle.shutdown_tx.send(true);
        let mut task = handle.task;
        if timeout(OUTBOX_STOP_TIMEOUT, &mut task).await.is_err() {
            task.abort();
            let _ = task.await;
        }
    }
}

async fn run_outbox_dispatcher(
    storage_runtime: Arc<crate::storage_runtime::StorageRuntime>,
    sync_runtime: Arc<Mutex<SyncRuntime>>,
    config: Arc<RwLock<AppConfig>>,
    mut shutdown_rx: watch::Receiver<bool>,
) {
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

        let network_enabled = config.read().await.sync.network.enabled;
        if !network_enabled {
            continue;
        }

        let _ = dispatch_due_outbox_once(&storage_runtime, &sync_runtime).await;
    }
}

async fn dispatch_due_outbox_once(
    storage_runtime: &crate::storage_runtime::StorageRuntime,
    sync_runtime: &Arc<Mutex<SyncRuntime>>,
) -> AppResult<()> {
    let now_ms = now_millis_i64();
    let due = storage_runtime
        .list_due_outbox(now_ms, OUTBOX_BATCH_SIZE)
        .await?;

    for message in due {
        let now_ms = now_millis_i64();
        let lease_until_ms = now_ms.saturating_add(OUTBOX_LEASE_MS);
        let leased = storage_runtime
            .try_lease_outbox_message(message.id, lease_until_ms, now_ms)
            .await?;
        if !leased {
            continue;
        }

        match dispatch_one_outbox_message(sync_runtime, &message).await {
            Ok(()) => {
                let _ = storage_runtime
                    .mark_outbox_sent(message.id, now_millis_i64())
                    .await?;
            }
            Err(error) => {
                let delay_ms = retry_delay_ms(message.attempt_count, error.engine_unavailable);
                let next_attempt_at_ms = now_ms.saturating_add(delay_ms);
                let _ = storage_runtime
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
    sync_runtime: &Arc<Mutex<SyncRuntime>>,
    message: &OutboxMessage,
) -> Result<(), DispatchFailure> {
    let (text_tx, connected_peer_ids) = {
        let runtime = sync_runtime.lock().await;
        match runtime.text_sender() {
            Ok(tx) => {
                let peer_ids = runtime
                    .connected_peers()
                    .into_iter()
                    .map(|peer| peer.peer_node_id)
                    .collect::<Vec<_>>();
                (tx, peer_ids)
            }
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
        }
    };

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
