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
                origin_noob_id,
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
                        origin_noob_id.as_deref(),
                        origin_device_id.as_deref(),
                        created_at_ms,
                        applied_at_ms,
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
            StorageCommand::GetEventById { event_id, reply } => {
                let result = state
                    .repository
                    .get_event_by_id(event_id)
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
