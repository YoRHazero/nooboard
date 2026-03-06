use std::path::Path;
use std::sync::Arc;

use tokio::sync::{mpsc, oneshot};

use crate::clipboard_runtime::{ClipboardPort, ClipboardRuntime, LocalClipboardSubscription};
use crate::config::AppConfig;
use crate::service::events::SubscriptionHub;
use crate::storage_runtime::StorageRuntime;
use crate::sync_runtime::SyncRuntime;
use crate::{AppError, AppResult};

use super::types::{
    AppPatch, AppServiceSnapshot, EventId, EventSubscription, FileDecisionRequest, HistoryPage,
    ListHistoryRequest, LocalClipboardChangeRequest, LocalClipboardChangeResult,
    RebroadcastHistoryRequest, RemoteTextRequest, SendFileRequest, SyncDesiredState,
};

mod control;

use control::{ControlCommand, ControlState, spawn_control_actor};

#[allow(async_fn_in_trait)]
pub trait AppService {
    async fn shutdown(&self) -> AppResult<()>;
    async fn set_sync_desired_state(
        &self,
        desired_state: SyncDesiredState,
    ) -> AppResult<AppServiceSnapshot>;
    async fn apply_config_patch(&self, patch: AppPatch) -> AppResult<AppServiceSnapshot>;
    async fn snapshot(&self) -> AppResult<AppServiceSnapshot>;

    async fn apply_local_clipboard_change(
        &self,
        request: LocalClipboardChangeRequest,
    ) -> AppResult<LocalClipboardChangeResult>;
    async fn apply_history_entry_to_clipboard(&self, event_id: EventId) -> AppResult<()>;
    async fn list_history(&self, request: ListHistoryRequest) -> AppResult<HistoryPage>;
    async fn rebroadcast_history_entry(&self, request: RebroadcastHistoryRequest) -> AppResult<()>;
    async fn store_remote_text(&self, request: RemoteTextRequest) -> AppResult<()>;
    async fn write_remote_text_to_clipboard(&self, request: RemoteTextRequest) -> AppResult<()>;

    async fn send_file(&self, request: SendFileRequest) -> AppResult<()>;
    async fn respond_file_decision(&self, request: FileDecisionRequest) -> AppResult<()>;

    async fn subscribe_events(&self) -> AppResult<EventSubscription>;
    async fn subscribe_local_clipboard(&self) -> AppResult<LocalClipboardSubscription>;
}

pub struct AppServiceImpl {
    command_tx: mpsc::Sender<ControlCommand>,
}

impl AppServiceImpl {
    pub fn new(
        config_path: impl AsRef<Path>,
        clipboard: Arc<dyn ClipboardPort>,
    ) -> AppResult<Self> {
        let config_path = config_path.as_ref().to_path_buf();
        let config = AppConfig::load(&config_path)?;
        let storage_runtime = Arc::new(StorageRuntime::new(config.to_storage_config())?);
        let clipboard = ClipboardRuntime::new(clipboard);
        let sync_runtime = SyncRuntime::new();
        let subscriptions = Arc::new(SubscriptionHub::new());

        let state = ControlState::new(
            config_path,
            config,
            storage_runtime,
            clipboard,
            sync_runtime,
            subscriptions,
        );

        Ok(Self {
            command_tx: spawn_control_actor(state),
        })
    }

    async fn request<T>(
        &self,
        command_factory: impl FnOnce(oneshot::Sender<AppResult<T>>) -> ControlCommand,
        op: &'static str,
    ) -> AppResult<T> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.command_tx
            .send(command_factory(reply_tx))
            .await
            .map_err(|_| {
                AppError::ChannelClosed(format!("control command channel closed: {op}"))
            })?;

        reply_rx.await.map_err(|_| {
            AppError::ChannelClosed(format!("control response channel closed: {op}"))
        })?
    }
}

impl AppService for AppServiceImpl {
    async fn shutdown(&self) -> AppResult<()> {
        self.request(|reply| ControlCommand::Shutdown { reply }, "shutdown")
            .await
    }

    async fn set_sync_desired_state(
        &self,
        desired_state: SyncDesiredState,
    ) -> AppResult<AppServiceSnapshot> {
        self.request(
            |reply| ControlCommand::SetSyncDesiredState {
                desired_state,
                reply,
            },
            "set_sync_desired_state",
        )
        .await
    }

    async fn apply_config_patch(&self, patch: AppPatch) -> AppResult<AppServiceSnapshot> {
        self.request(
            |reply| ControlCommand::ApplyConfigPatch { patch, reply },
            "apply_config_patch",
        )
        .await
    }

    async fn snapshot(&self) -> AppResult<AppServiceSnapshot> {
        self.request(|reply| ControlCommand::Snapshot { reply }, "snapshot")
            .await
    }

    async fn apply_local_clipboard_change(
        &self,
        request: LocalClipboardChangeRequest,
    ) -> AppResult<LocalClipboardChangeResult> {
        self.request(
            |reply| ControlCommand::ApplyLocalClipboardChange { request, reply },
            "apply_local_clipboard_change",
        )
        .await
    }

    async fn apply_history_entry_to_clipboard(&self, event_id: EventId) -> AppResult<()> {
        self.request(
            |reply| ControlCommand::ApplyHistoryEntryToClipboard { event_id, reply },
            "apply_history_entry_to_clipboard",
        )
        .await
    }

    async fn list_history(&self, request: ListHistoryRequest) -> AppResult<HistoryPage> {
        self.request(
            |reply| ControlCommand::ListHistory { request, reply },
            "list_history",
        )
        .await
    }

    async fn rebroadcast_history_entry(&self, request: RebroadcastHistoryRequest) -> AppResult<()> {
        self.request(
            |reply| ControlCommand::RebroadcastHistoryEntry { request, reply },
            "rebroadcast_history_entry",
        )
        .await
    }

    async fn store_remote_text(&self, request: RemoteTextRequest) -> AppResult<()> {
        self.request(
            |reply| ControlCommand::StoreRemoteText { request, reply },
            "store_remote_text",
        )
        .await
    }

    async fn write_remote_text_to_clipboard(&self, request: RemoteTextRequest) -> AppResult<()> {
        self.request(
            |reply| ControlCommand::WriteRemoteTextToClipboard { request, reply },
            "write_remote_text_to_clipboard",
        )
        .await
    }

    async fn send_file(&self, request: SendFileRequest) -> AppResult<()> {
        self.request(
            |reply| ControlCommand::SendFile { request, reply },
            "send_file",
        )
        .await
    }

    async fn respond_file_decision(&self, request: FileDecisionRequest) -> AppResult<()> {
        self.request(
            |reply| ControlCommand::RespondFileDecision { request, reply },
            "respond_file_decision",
        )
        .await
    }

    async fn subscribe_events(&self) -> AppResult<EventSubscription> {
        self.request(
            |reply| ControlCommand::SubscribeEvents { reply },
            "subscribe_events",
        )
        .await
    }

    async fn subscribe_local_clipboard(&self) -> AppResult<LocalClipboardSubscription> {
        self.request(
            |reply| ControlCommand::SubscribeLocalClipboard { reply },
            "subscribe_local_clipboard",
        )
        .await
    }
}
