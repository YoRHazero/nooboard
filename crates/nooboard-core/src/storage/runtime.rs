use std::sync::{Mutex, mpsc};
use std::thread::JoinHandle;

use nooboard_storage::{
    HistoryPage, HistoryRecord, HistoryRecordSource, ListHistoryRequest, SqliteEventRepository,
};
use tokio::sync::oneshot;
use uuid::Uuid;

use crate::error::{CoreError, CoreResult};
use crate::types::EventId;

enum StorageCommand {
    Reconfigure {
        storage_config: nooboard_storage::AppConfig,
        reply: oneshot::Sender<CoreResult<()>>,
    },
    AppendText {
        text: String,
        event_id: Option<Uuid>,
        origin_noob_id: Option<String>,
        origin_device_id: Option<String>,
        created_at_ms: i64,
        applied_at_ms: i64,
        source: HistoryRecordSource,
        reply: oneshot::Sender<CoreResult<bool>>,
    },
    ListHistory {
        request: ListHistoryRequest,
        reply: oneshot::Sender<CoreResult<HistoryPage>>,
    },
    GetEventById {
        event_id: Uuid,
        reply: oneshot::Sender<CoreResult<Option<HistoryRecord>>>,
    },
    Shutdown,
}

pub(crate) struct StorageRuntime {
    command_tx: mpsc::Sender<StorageCommand>,
    runtime_handle: tokio::runtime::Handle,
    worker: Mutex<Option<JoinHandle<()>>>,
}

impl StorageRuntime {
    pub(crate) fn new(
        storage_config: nooboard_storage::AppConfig,
        runtime_handle: tokio::runtime::Handle,
    ) -> CoreResult<(Self, Option<EventId>)> {
        let (command_tx, command_rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);

        let worker = std::thread::Builder::new()
            .name("nooboard-core-storage".to_string())
            .spawn(move || run_actor(storage_config, command_rx, ready_tx))
            .map_err(|error| {
                CoreError::ChannelClosed(format!("failed to spawn storage actor: {error}"))
            })?;

        match ready_rx.recv() {
            Ok(Ok(latest_event_id)) => Ok((
                Self {
                    command_tx,
                    runtime_handle,
                    worker: Mutex::new(Some(worker)),
                },
                latest_event_id,
            )),
            Ok(Err(error)) => {
                let _ = worker.join();
                Err(error)
            }
            Err(error) => {
                let _ = worker.join();
                Err(CoreError::ChannelClosed(format!(
                    "storage actor startup signal dropped: {error}"
                )))
            }
        }
    }

    pub(crate) async fn reconfigure(
        &self,
        storage_config: nooboard_storage::AppConfig,
    ) -> CoreResult<()> {
        self.request(
            |reply| StorageCommand::Reconfigure {
                storage_config,
                reply,
            },
            "reconfigure",
        )
        .await
    }

    pub(crate) async fn append_text_with_source(
        &self,
        text: &str,
        event_id: Option<Uuid>,
        origin_noob_id: Option<&str>,
        origin_device_id: Option<&str>,
        created_at_ms: i64,
        applied_at_ms: i64,
        source: HistoryRecordSource,
    ) -> CoreResult<bool> {
        self.request(
            |reply| StorageCommand::AppendText {
                text: text.to_string(),
                event_id,
                origin_noob_id: origin_noob_id.map(ToString::to_string),
                origin_device_id: origin_device_id.map(ToString::to_string),
                created_at_ms,
                applied_at_ms,
                source,
                reply,
            },
            "append_text",
        )
        .await
    }

    pub(crate) async fn list_history(
        &self,
        request: ListHistoryRequest,
    ) -> CoreResult<HistoryPage> {
        self.request(
            |reply| StorageCommand::ListHistory { request, reply },
            "list_history",
        )
        .await
    }

    pub(crate) async fn get_event_by_id(
        &self,
        event_id: Uuid,
    ) -> CoreResult<Option<HistoryRecord>> {
        self.request(
            |reply| StorageCommand::GetEventById { event_id, reply },
            "get_event_by_id",
        )
        .await
    }

    pub(crate) async fn shutdown(&self) -> CoreResult<()> {
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
        let join_result = self
            .runtime_handle
            .spawn_blocking(move || worker.join())
            .await
            .map_err(|error| {
                CoreError::ChannelClosed(format!("failed to join storage actor thread: {error}"))
            })?;
        if join_result.is_err() {
            return Err(CoreError::ChannelClosed(
                "storage actor thread panicked while shutting down".to_string(),
            ));
        }
        Ok(())
    }

    async fn request<T>(
        &self,
        command_factory: impl FnOnce(oneshot::Sender<CoreResult<T>>) -> StorageCommand,
        op: &'static str,
    ) -> CoreResult<T> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.command_tx
            .send(command_factory(reply_tx))
            .map_err(|_| {
                CoreError::ChannelClosed(format!("storage command channel closed: {op}"))
            })?;
        reply_rx.await.map_err(|_| {
            CoreError::ChannelClosed(format!("storage response channel closed: {op}"))
        })?
    }
}

impl Drop for StorageRuntime {
    fn drop(&mut self) {
        let _ = self.command_tx.send(StorageCommand::Shutdown);
    }
}

fn run_actor(
    storage_config: nooboard_storage::AppConfig,
    command_rx: mpsc::Receiver<StorageCommand>,
    ready_tx: mpsc::SyncSender<CoreResult<Option<EventId>>>,
) {
    let (mut state, latest_event_id) = match ActorState::new(storage_config) {
        Ok(value) => value,
        Err(error) => {
            let _ = ready_tx.send(Err(error));
            return;
        }
    };
    let _ = ready_tx.send(Ok(latest_event_id));

    while let Ok(command) = command_rx.recv() {
        match command {
            StorageCommand::Reconfigure {
                storage_config,
                reply,
            } => {
                let _ = reply.send(state.reconfigure(storage_config));
            }
            StorageCommand::AppendText {
                text,
                event_id,
                origin_noob_id,
                origin_device_id,
                created_at_ms,
                applied_at_ms,
                source,
                reply,
            } => {
                let result = state
                    .repository
                    .append_text_with_source(
                        &text,
                        event_id,
                        origin_noob_id.as_deref(),
                        origin_device_id.as_deref(),
                        created_at_ms,
                        applied_at_ms,
                        source,
                    )
                    .map_err(Into::into);
                let _ = reply.send(result);
            }
            StorageCommand::ListHistory { request, reply } => {
                let result = state.repository.list_history(request).map_err(Into::into);
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
}

impl ActorState {
    fn new(storage_config: nooboard_storage::AppConfig) -> CoreResult<(Self, Option<EventId>)> {
        let repository = open_repository(&storage_config)?;
        let latest_event_id = repository
            .list_history(ListHistoryRequest {
                limit: 1,
                direction: nooboard_storage::HistoryDirection::Older,
                anchor: None,
            })?
            .records
            .into_iter()
            .next()
            .map(|record| EventId::from(Uuid::from_bytes(record.event_id)));
        Ok((Self { repository }, latest_event_id))
    }

    fn reconfigure(&mut self, storage_config: nooboard_storage::AppConfig) -> CoreResult<()> {
        self.repository = open_repository(&storage_config)?;
        Ok(())
    }
}

fn open_repository(
    storage_config: &nooboard_storage::AppConfig,
) -> Result<SqliteEventRepository, CoreError> {
    let mut repository = SqliteEventRepository::open(storage_config.clone())?;
    repository.init_storage()?;
    Ok(repository)
}
