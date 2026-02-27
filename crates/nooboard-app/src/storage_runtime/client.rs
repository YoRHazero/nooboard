use std::sync::{Mutex, mpsc};
use std::thread::JoinHandle;

use nooboard_storage::{HistoryCursor, HistoryRecord, OutboxMessage};
use tokio::sync::oneshot;
use uuid::Uuid;

use crate::{AppError, AppResult};

use super::actor::run_actor;
use super::commands::StorageCommand;

pub(crate) struct StorageRuntime {
    command_tx: mpsc::Sender<StorageCommand>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

impl StorageRuntime {
    pub(crate) fn new(storage_config: nooboard_storage::AppConfig) -> AppResult<Self> {
        let (command_tx, command_rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);

        let worker = std::thread::Builder::new()
            .name("nooboard-storage-runtime".to_string())
            .spawn(move || run_actor(storage_config, command_rx, ready_tx))
            .map_err(|error| {
                AppError::ChannelClosed(format!("failed to spawn storage actor: {error}"))
            })?;

        match ready_rx.recv() {
            Ok(Ok(())) => Ok(Self {
                command_tx,
                worker: Mutex::new(Some(worker)),
            }),
            Ok(Err(error)) => {
                let _ = worker.join();
                Err(error)
            }
            Err(error) => {
                let _ = worker.join();
                Err(AppError::ChannelClosed(format!(
                    "storage actor startup signal dropped: {error}"
                )))
            }
        }
    }

    pub(crate) async fn reconfigure(
        &self,
        storage_config: nooboard_storage::AppConfig,
    ) -> AppResult<()> {
        self.request(
            |reply| StorageCommand::Reconfigure {
                storage_config,
                reply,
            },
            "reconfigure",
        )
        .await
    }

    pub(crate) async fn append_text(
        &self,
        text: &str,
        event_id: Option<Uuid>,
        origin_device_id: Option<&str>,
        created_at_ms: i64,
        applied_at_ms: i64,
    ) -> AppResult<bool> {
        self.request(
            |reply| StorageCommand::AppendText {
                text: text.to_string(),
                event_id,
                origin_device_id: origin_device_id.map(ToString::to_string),
                created_at_ms,
                applied_at_ms,
                reply,
            },
            "append_text",
        )
        .await
    }

    pub(crate) async fn list_history(
        &self,
        limit: usize,
        cursor: Option<HistoryCursor>,
    ) -> AppResult<Vec<HistoryRecord>> {
        self.request(
            |reply| StorageCommand::ListHistory {
                limit,
                cursor,
                reply,
            },
            "list_history",
        )
        .await
    }

    pub(crate) async fn append_text_with_outbox(
        &self,
        text: &str,
        event_id: Uuid,
        origin_device_id: Option<&str>,
        created_at_ms: i64,
        applied_at_ms: i64,
        targets: Option<Vec<String>>,
        enqueue_at_ms: i64,
    ) -> AppResult<bool> {
        self.request(
            |reply| StorageCommand::AppendTextWithOutbox {
                text: text.to_string(),
                event_id,
                origin_device_id: origin_device_id.map(ToString::to_string),
                created_at_ms,
                applied_at_ms,
                targets,
                enqueue_at_ms,
                reply,
            },
            "append_text_with_outbox",
        )
        .await
    }

    pub(crate) async fn list_due_outbox(
        &self,
        now_ms: i64,
        limit: usize,
    ) -> AppResult<Vec<OutboxMessage>> {
        self.request(
            |reply| StorageCommand::ListDueOutbox {
                now_ms,
                limit,
                reply,
            },
            "list_due_outbox",
        )
        .await
    }

    pub(crate) async fn try_lease_outbox_message(
        &self,
        id: i64,
        lease_until_ms: i64,
        now_ms: i64,
    ) -> AppResult<bool> {
        self.request(
            |reply| StorageCommand::TryLeaseOutbox {
                id,
                lease_until_ms,
                now_ms,
                reply,
            },
            "try_lease_outbox_message",
        )
        .await
    }

    pub(crate) async fn mark_outbox_sent(&self, id: i64, sent_at_ms: i64) -> AppResult<bool> {
        self.request(
            |reply| StorageCommand::MarkOutboxSent {
                id,
                sent_at_ms,
                reply,
            },
            "mark_outbox_sent",
        )
        .await
    }

    pub(crate) async fn mark_outbox_retry(
        &self,
        id: i64,
        next_attempt_at_ms: i64,
        error: String,
        now_ms: i64,
    ) -> AppResult<bool> {
        self.request(
            |reply| StorageCommand::MarkOutboxRetry {
                id,
                next_attempt_at_ms,
                error,
                now_ms,
                reply,
            },
            "mark_outbox_retry",
        )
        .await
    }

    pub(crate) async fn shutdown(&self) -> AppResult<()> {
        let worker = {
            let mut guard = match self.worker.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            guard.take()
        };
        let Some(worker) = worker else {
            return Ok(());
        };

        let _ = self.command_tx.send(StorageCommand::Shutdown);
        let join_result = tokio::task::spawn_blocking(move || worker.join())
            .await
            .map_err(|error| {
                AppError::ChannelClosed(format!("failed to join storage actor thread: {error}"))
            })?;
        if join_result.is_err() {
            return Err(AppError::ChannelClosed(
                "storage actor thread panicked while shutting down".to_string(),
            ));
        }
        Ok(())
    }

    async fn request<T>(
        &self,
        command_factory: impl FnOnce(oneshot::Sender<AppResult<T>>) -> StorageCommand,
        op: &'static str,
    ) -> AppResult<T> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.command_tx
            .send(command_factory(reply_tx))
            .map_err(|_| {
                AppError::ChannelClosed(format!("storage command channel closed: {op}"))
            })?;
        reply_rx.await.map_err(|_| {
            AppError::ChannelClosed(format!("storage response channel closed: {op}"))
        })?
    }
}

impl Drop for StorageRuntime {
    fn drop(&mut self) {
        let _ = self.command_tx.send(StorageCommand::Shutdown);
    }
}
