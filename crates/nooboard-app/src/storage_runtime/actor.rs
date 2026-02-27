use std::sync::mpsc;

use nooboard_storage::SqliteEventRepository;

use crate::AppResult;

use super::commands::StorageCommand;
use super::repository::open_repository;
use super::signature::StorageConfigSignature;

pub(super) fn run_actor(
    storage_config: nooboard_storage::AppConfig,
    command_rx: mpsc::Receiver<StorageCommand>,
    ready_tx: mpsc::SyncSender<AppResult<()>>,
) {
    let mut state = match ActorState::new(storage_config) {
        Ok(state) => {
            let _ = ready_tx.send(Ok(()));
            state
        }
        Err(error) => {
            let _ = ready_tx.send(Err(error));
            return;
        }
    };

    while let Ok(command) = command_rx.recv() {
        match command {
            StorageCommand::Reconfigure {
                storage_config,
                reply,
            } => {
                let result = state.reconfigure(storage_config);
                let _ = reply.send(result);
            }
            StorageCommand::AppendText {
                text,
                event_id,
                origin_device_id,
                created_at_ms,
                applied_at_ms,
                reply,
            } => {
                let result = state
                    .repository
                    .append_text(
                        &text,
                        event_id,
                        origin_device_id.as_deref(),
                        created_at_ms,
                        applied_at_ms,
                    )
                    .map_err(Into::into);
                let _ = reply.send(result);
            }
            StorageCommand::AppendTextWithOutbox {
                text,
                event_id,
                origin_device_id,
                created_at_ms,
                applied_at_ms,
                targets,
                enqueue_at_ms,
                reply,
            } => {
                let result = state
                    .repository
                    .append_text_with_outbox(
                        &text,
                        event_id,
                        origin_device_id.as_deref(),
                        created_at_ms,
                        applied_at_ms,
                        targets.as_deref(),
                        enqueue_at_ms,
                    )
                    .map_err(Into::into);
                let _ = reply.send(result);
            }
            StorageCommand::ListHistory {
                limit,
                cursor,
                reply,
            } => {
                let result = state
                    .repository
                    .list_history(limit, cursor)
                    .map_err(Into::into);
                let _ = reply.send(result);
            }
            StorageCommand::ListDueOutbox {
                now_ms,
                limit,
                reply,
            } => {
                let result = state
                    .repository
                    .list_due_outbox(now_ms, limit)
                    .map_err(Into::into);
                let _ = reply.send(result);
            }
            StorageCommand::TryLeaseOutbox {
                id,
                lease_until_ms,
                now_ms,
                reply,
            } => {
                let result = state
                    .repository
                    .try_lease_outbox_message(id, lease_until_ms, now_ms)
                    .map_err(Into::into);
                let _ = reply.send(result);
            }
            StorageCommand::MarkOutboxSent {
                id,
                sent_at_ms,
                reply,
            } => {
                let result = state
                    .repository
                    .mark_outbox_sent(id, sent_at_ms)
                    .map_err(Into::into);
                let _ = reply.send(result);
            }
            StorageCommand::MarkOutboxRetry {
                id,
                next_attempt_at_ms,
                error,
                now_ms,
                reply,
            } => {
                let result = state
                    .repository
                    .mark_outbox_retry(id, next_attempt_at_ms, &error, now_ms)
                    .map_err(Into::into);
                let _ = reply.send(result);
            }
            StorageCommand::Shutdown => break,
        }
    }
}

struct ActorState {
    repository: SqliteEventRepository,
    signature: StorageConfigSignature,
}

impl ActorState {
    fn new(storage_config: nooboard_storage::AppConfig) -> AppResult<Self> {
        let signature = StorageConfigSignature::from_config(&storage_config);
        let repository = open_repository(&storage_config)?;
        Ok(Self {
            repository,
            signature,
        })
    }

    fn reconfigure(&mut self, storage_config: nooboard_storage::AppConfig) -> AppResult<()> {
        let signature = StorageConfigSignature::from_config(&storage_config);
        if self.signature == signature {
            return Ok(());
        }

        self.repository = open_repository(&storage_config)?;
        self.signature = signature;
        Ok(())
    }
}
