use std::sync::Arc;
use std::sync::Mutex;

use tokio::sync::{mpsc, oneshot};

use crate::bootstrap::{BootstrapLaunch, prepare_bootstrap_launch};
use crate::clipboard::{ClipboardRuntime, port::ClipboardPort};
use crate::error::{CoreError, CoreResult};
use crate::storage::StorageRuntime;
use crate::types::{
    EventId, EventSubscription, LocalConnectionInfo, StateSubscription, WorkspaceSnapshot,
};
use crate::workspace::{WorkspaceState, spawn_workspace_actor};
use crate::{IncomingTransferDecision, SendFilesRequest, SessionTarget, StorageSettingsInput};

#[derive(Clone)]
pub struct NooboardCore {
    inner: Arc<CoreInner>,
}

struct CoreInner {
    command_tx: mpsc::Sender<crate::workspace::command::WorkspaceCommand>,
    runtime: Mutex<Option<tokio::runtime::Runtime>>,
    task: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl NooboardCore {
    pub fn launch(launch: &BootstrapLaunch, clipboard: Arc<dyn ClipboardPort>) -> CoreResult<Self> {
        prepare_bootstrap_launch(launch)?;
        let config_path = launch.config_path.clone();
        let config = nooboard_config::AppConfig::load(&config_path)?;
        let runtime = build_internal_runtime()?;
        let runtime_handle = runtime.handle().clone();
        let (storage_runtime, latest_event_id) =
            StorageRuntime::new(config.to_storage_config(), runtime_handle.clone())?;
        let clipboard_runtime = ClipboardRuntime::new(clipboard, runtime_handle.clone());
        let network_runtime = nooboard_network::NetworkRuntime::new(config.to_network_config()?)?;
        let network_snapshot = network_runtime.snapshot();
        let clipboard_subscription = clipboard_runtime.subscribe_local_changes();
        let network_subscription = network_runtime.subscribe();
        if config.local_capture_enabled() {
            clipboard_runtime.start_watch()?;
        }
        let state = WorkspaceState::new(
            config_path,
            config.clone(),
            storage_runtime,
            clipboard_runtime,
            network_runtime,
            LocalConnectionInfo {
                device_endpoint: crate::workspace::local_connection::detect_device_endpoint(
                    config.network.listen_port,
                ),
            },
            latest_event_id,
            network_snapshot,
        );
        let actor = spawn_workspace_actor(
            &runtime_handle,
            state,
            clipboard_subscription,
            network_subscription,
        );

        Ok(Self {
            inner: Arc::new(CoreInner {
                command_tx: actor.command_tx,
                runtime: Mutex::new(Some(runtime)),
                task: Mutex::new(Some(actor.task)),
            }),
        })
    }

    pub async fn shutdown(&self) -> CoreResult<()> {
        let task = take_mutex_option(&self.inner.task);
        if task.is_none() {
            drop_runtime(take_mutex_option(&self.inner.runtime)).await?;
            return Ok(());
        }
        let task = task.expect("checked is_some");

        let result = self
            .request(
                |reply| crate::workspace::command::WorkspaceCommand::Shutdown { reply },
                "shutdown",
            )
            .await;
        let join_result = task.await.map_err(|error| {
            CoreError::ChannelClosed(format!("workspace actor task join failed: {error}"))
        });
        drop_runtime(take_mutex_option(&self.inner.runtime)).await?;

        match result {
            Ok(()) => join_result?,
            Err(error) => return Err(error),
        }
        Ok(())
    }

    pub async fn snapshot(&self) -> CoreResult<WorkspaceSnapshot> {
        self.request(
            |reply| crate::workspace::command::WorkspaceCommand::Snapshot { reply },
            "snapshot",
        )
        .await
    }

    pub async fn subscribe_state(&self) -> CoreResult<StateSubscription> {
        self.request(
            |reply| crate::workspace::command::WorkspaceCommand::SubscribeState { reply },
            "subscribe_state",
        )
        .await
    }

    pub async fn subscribe_events(&self) -> CoreResult<EventSubscription> {
        self.request(
            |reply| crate::workspace::command::WorkspaceCommand::SubscribeEvents { reply },
            "subscribe_events",
        )
        .await
    }

    pub async fn start_network(&self) -> CoreResult<()> {
        self.request(
            |reply| crate::workspace::command::WorkspaceCommand::StartNetwork { reply },
            "start_network",
        )
        .await
    }

    pub async fn stop_network(&self) -> CoreResult<()> {
        self.request(
            |reply| crate::workspace::command::WorkspaceCommand::StopNetwork { reply },
            "stop_network",
        )
        .await
    }

    pub async fn set_device_id(&self, value: String) -> CoreResult<()> {
        self.request(
            |reply| crate::workspace::command::WorkspaceCommand::SetDeviceId { value, reply },
            "set_device_id",
        )
        .await
    }

    pub async fn set_network_token(&self, value: String) -> CoreResult<()> {
        self.request(
            |reply| crate::workspace::command::WorkspaceCommand::SetNetworkToken { value, reply },
            "set_network_token",
        )
        .await
    }

    pub async fn set_network_listen_port(&self, value: u16) -> CoreResult<()> {
        self.request(
            |reply| crate::workspace::command::WorkspaceCommand::SetNetworkListenPort {
                value,
                reply,
            },
            "set_network_listen_port",
        )
        .await
    }

    pub async fn set_lan_enabled(&self, value: bool) -> CoreResult<()> {
        self.request(
            |reply| crate::workspace::command::WorkspaceCommand::SetLanEnabled { value, reply },
            "set_lan_enabled",
        )
        .await
    }

    pub async fn set_local_capture_enabled(&self, value: bool) -> CoreResult<()> {
        self.request(
            |reply| crate::workspace::command::WorkspaceCommand::SetLocalCaptureEnabled {
                value,
                reply,
            },
            "set_local_capture_enabled",
        )
        .await
    }

    pub async fn set_download_dir(&self, value: std::path::PathBuf) -> CoreResult<()> {
        self.request(
            |reply| crate::workspace::command::WorkspaceCommand::SetDownloadDir { value, reply },
            "set_download_dir",
        )
        .await
    }

    pub async fn set_storage_settings(&self, input: StorageSettingsInput) -> CoreResult<()> {
        self.request(
            |reply| crate::workspace::command::WorkspaceCommand::SetStorageSettings {
                input,
                reply,
            },
            "set_storage_settings",
        )
        .await
    }

    pub async fn list_direct_seeds(&self) -> CoreResult<Vec<crate::DirectSeedInfo>> {
        self.request(
            |reply| crate::workspace::command::WorkspaceCommand::ListDirectSeeds { reply },
            "list_direct_seeds",
        )
        .await
    }

    pub async fn upsert_direct_seed(
        &self,
        input: crate::UpsertDirectSeedInput,
    ) -> CoreResult<crate::DirectSeedId> {
        self.request(
            |reply| crate::workspace::command::WorkspaceCommand::UpsertDirectSeed { input, reply },
            "upsert_direct_seed",
        )
        .await
    }

    pub async fn remove_direct_seed(&self, id: crate::DirectSeedId) -> CoreResult<()> {
        self.request(
            |reply| crate::workspace::command::WorkspaceCommand::RemoveDirectSeed { id, reply },
            "remove_direct_seed",
        )
        .await
    }

    pub async fn search_direct_seeds(&self, query: &str) -> CoreResult<Vec<crate::DirectSeedInfo>> {
        self.request(
            |reply| crate::workspace::command::WorkspaceCommand::SearchDirectSeeds {
                query: query.to_string(),
                reply,
            },
            "search_direct_seeds",
        )
        .await
    }

    pub async fn connect_direct_seed(
        &self,
        id: crate::DirectSeedId,
    ) -> CoreResult<crate::ConnectDirectOutcome> {
        self.request(
            |reply| crate::workspace::command::WorkspaceCommand::ConnectDirectSeed { id, reply },
            "connect_direct_seed",
        )
        .await
    }

    pub async fn list_pending_direct_requests(
        &self,
    ) -> CoreResult<Vec<crate::PendingDirectRequest>> {
        self.request(
            |reply| crate::workspace::command::WorkspaceCommand::ListPendingDirectRequests {
                reply,
            },
            "list_pending_direct_requests",
        )
        .await
    }

    pub async fn approve_direct_request(&self, id: crate::DirectRequestId) -> CoreResult<()> {
        self.request(
            |reply| crate::workspace::command::WorkspaceCommand::ApproveDirectRequest { id, reply },
            "approve_direct_request",
        )
        .await
    }

    pub async fn reject_direct_request(&self, id: crate::DirectRequestId) -> CoreResult<()> {
        self.request(
            |reply| crate::workspace::command::WorkspaceCommand::RejectDirectRequest { id, reply },
            "reject_direct_request",
        )
        .await
    }

    pub async fn list_sessions(&self) -> CoreResult<Vec<crate::SessionInfo>> {
        self.request(
            |reply| crate::workspace::command::WorkspaceCommand::ListSessions { reply },
            "list_sessions",
        )
        .await
    }

    pub async fn disconnect_session(&self, id: crate::SessionId) -> CoreResult<()> {
        self.request(
            |reply| crate::workspace::command::WorkspaceCommand::DisconnectSession { id, reply },
            "disconnect_session",
        )
        .await
    }

    pub async fn submit_text(&self, content: String) -> CoreResult<EventId> {
        self.request(
            |reply| crate::workspace::command::WorkspaceCommand::SubmitText { content, reply },
            "submit_text",
        )
        .await
    }

    pub async fn get_clipboard_record(
        &self,
        event_id: EventId,
    ) -> CoreResult<crate::ClipboardRecord> {
        self.request(
            |reply| crate::workspace::command::WorkspaceCommand::GetClipboardRecord {
                event_id,
                reply,
            },
            "get_clipboard_record",
        )
        .await
    }

    pub async fn list_clipboard_history(
        &self,
        request: crate::ListClipboardHistoryRequest,
    ) -> CoreResult<crate::ClipboardHistoryPage> {
        self.request(
            |reply| crate::workspace::command::WorkspaceCommand::ListClipboardHistory {
                request,
                reply,
            },
            "list_clipboard_history",
        )
        .await
    }

    pub async fn adopt_clipboard_record(&self, event_id: EventId) -> CoreResult<()> {
        self.request(
            |reply| crate::workspace::command::WorkspaceCommand::AdoptClipboardRecord {
                event_id,
                reply,
            },
            "adopt_clipboard_record",
        )
        .await
    }

    pub async fn rebroadcast_clipboard_record(
        &self,
        event_id: EventId,
        target: SessionTarget,
    ) -> CoreResult<()> {
        self.request(
            |reply| crate::workspace::command::WorkspaceCommand::RebroadcastClipboardRecord {
                event_id,
                target,
                reply,
            },
            "rebroadcast_clipboard_record",
        )
        .await
    }

    pub async fn send_files(
        &self,
        request: SendFilesRequest,
    ) -> CoreResult<Vec<crate::TransferTicket>> {
        self.request(
            |reply| crate::workspace::command::WorkspaceCommand::SendFiles { request, reply },
            "send_files",
        )
        .await
    }

    pub async fn decide_incoming_transfer(
        &self,
        decision: IncomingTransferDecision,
    ) -> CoreResult<()> {
        self.request(
            |reply| crate::workspace::command::WorkspaceCommand::DecideIncomingTransfer {
                decision,
                reply,
            },
            "decide_incoming_transfer",
        )
        .await
    }

    pub async fn cancel_transfer(&self, ticket: crate::TransferTicket) -> CoreResult<()> {
        self.request(
            |reply| crate::workspace::command::WorkspaceCommand::CancelTransfer { ticket, reply },
            "cancel_transfer",
        )
        .await
    }

    async fn request<T>(
        &self,
        command_factory: impl FnOnce(
            oneshot::Sender<CoreResult<T>>,
        ) -> crate::workspace::command::WorkspaceCommand,
        op: &'static str,
    ) -> CoreResult<T> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.inner
            .command_tx
            .send(command_factory(reply_tx))
            .await
            .map_err(|_| {
                CoreError::ChannelClosed(format!("workspace command channel closed: {op}"))
            })?;
        reply_rx.await.map_err(|_| {
            CoreError::ChannelClosed(format!("workspace response channel closed: {op}"))
        })?
    }
}

fn build_internal_runtime() -> CoreResult<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name("nooboard-core")
        .build()
        .map_err(|error| {
            CoreError::InvalidState(format!("failed to build nooboard-core runtime: {error}"))
        })
}

fn take_mutex_option<T>(mutex: &Mutex<Option<T>>) -> Option<T> {
    match mutex.lock() {
        Ok(mut guard) => guard.take(),
        Err(poisoned) => poisoned.into_inner().take(),
    }
}

async fn drop_runtime(runtime: Option<tokio::runtime::Runtime>) -> CoreResult<()> {
    let Some(runtime) = runtime else {
        return Ok(());
    };

    tokio::task::spawn_blocking(move || drop(runtime))
        .await
        .map_err(|error| {
            CoreError::ChannelClosed(format!(
                "failed to drop nooboard-core runtime on blocking task: {error}"
            ))
        })?;

    Ok(())
}
