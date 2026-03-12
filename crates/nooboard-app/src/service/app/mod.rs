use std::path::Path;
use std::sync::Arc;

use nooboard_config::{
    AppConfig, BootstrapLaunch, BootstrapRequest, prepare_bootstrap_launch, resolve_bootstrap,
};
use tokio::sync::{mpsc, oneshot};

use crate::clipboard_runtime::{ClipboardPort, ClipboardRuntime};
use crate::storage_runtime::StorageRuntime;
use crate::sync_runtime::SyncRuntime;
use crate::{AppError, AppResult};

use super::types::{
    AppState, ClipboardHistoryPage, ClipboardRecord, EventId, EventSubscription,
    IncomingTransferDecision, ListClipboardHistoryRequest, RebroadcastClipboardRequest,
    SendFilesRequest, SettingsPatch, StateSubscription, SubmitTextRequest, SyncDesiredState,
    TransferId,
};

mod control;

use control::{ControlCommand, ControlState, spawn_control_actor};

#[allow(async_fn_in_trait)]
pub trait DesktopAppService {
    async fn shutdown(&self) -> AppResult<()>;
    async fn get_state(&self) -> AppResult<AppState>;
    async fn subscribe_state(&self) -> AppResult<StateSubscription>;
    async fn subscribe_events(&self) -> AppResult<EventSubscription>;
    async fn set_sync_desired_state(&self, desired: SyncDesiredState) -> AppResult<()>;
    async fn patch_settings(&self, patch: SettingsPatch) -> AppResult<()>;
    async fn submit_text(&self, request: SubmitTextRequest) -> AppResult<EventId>;
    async fn get_clipboard_record(&self, event_id: EventId) -> AppResult<ClipboardRecord>;
    async fn list_clipboard_history(
        &self,
        request: ListClipboardHistoryRequest,
    ) -> AppResult<ClipboardHistoryPage>;
    async fn adopt_clipboard_record(&self, event_id: EventId) -> AppResult<()>;
    async fn rebroadcast_clipboard_record(
        &self,
        request: RebroadcastClipboardRequest,
    ) -> AppResult<()>;
    async fn send_files(&self, request: SendFilesRequest) -> AppResult<Vec<TransferId>>;
    async fn decide_incoming_transfer(&self, request: IncomingTransferDecision) -> AppResult<()>;
    async fn cancel_transfer(&self, transfer_id: TransferId) -> AppResult<()>;
}

pub struct DesktopAppServiceImpl {
    command_tx: mpsc::Sender<ControlCommand>,
}

impl DesktopAppServiceImpl {
    pub fn new_default() -> AppResult<Self> {
        match resolve_bootstrap(&BootstrapRequest::default())? {
            nooboard_config::BootstrapDecision::Launch(launch) => Self::new_with_launch(&launch),
            nooboard_config::BootstrapDecision::NeedsChooser(context) => {
                Err(AppError::InvalidConfig(format!(
                    "bootstrap chooser required before startup for {}",
                    context.default_config_path.display()
                )))
            }
        }
    }

    pub fn new_with_launch(launch: &BootstrapLaunch) -> AppResult<Self> {
        prepare_bootstrap_launch(launch)?;
        Self::new(&launch.config_path)
    }

    pub fn new(config_path: impl AsRef<Path>) -> AppResult<Self> {
        Self::new_with_clipboard(config_path, default_clipboard_port()?)
    }

    #[doc(hidden)]
    pub fn new_with_clipboard(
        config_path: impl AsRef<Path>,
        clipboard: Arc<dyn ClipboardPort>,
    ) -> AppResult<Self> {
        let config_path = config_path.as_ref().to_path_buf();
        let config = AppConfig::load(&config_path)?;
        let storage_runtime = Arc::new(StorageRuntime::new(config.to_storage_config())?);
        let clipboard = ClipboardRuntime::new(clipboard);
        let sync_runtime = SyncRuntime::new();
        let state = ControlState::new(
            config_path,
            config,
            storage_runtime,
            clipboard,
            sync_runtime,
            None,
            None,
        )?;

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

fn default_clipboard_port() -> AppResult<Arc<dyn ClipboardPort>> {
    #[cfg(target_os = "macos")]
    {
        return Ok(Arc::new(
            nooboard_platform_macos::MacOsClipboardBackend::new(),
        ));
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err(AppError::Platform(
            nooboard_platform::NooboardError::UnsupportedPlatform,
        ))
    }
}

impl DesktopAppService for DesktopAppServiceImpl {
    async fn shutdown(&self) -> AppResult<()> {
        self.request(|reply| ControlCommand::Shutdown { reply }, "shutdown")
            .await
    }

    async fn get_state(&self) -> AppResult<AppState> {
        self.request(|reply| ControlCommand::GetState { reply }, "get_state")
            .await
    }

    async fn subscribe_state(&self) -> AppResult<StateSubscription> {
        self.request(
            |reply| ControlCommand::SubscribeState { reply },
            "subscribe_state",
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

    async fn set_sync_desired_state(&self, desired: SyncDesiredState) -> AppResult<()> {
        self.request(
            |reply| ControlCommand::SetSyncDesiredState {
                desired_state: desired,
                reply,
            },
            "set_sync_desired_state",
        )
        .await
    }

    async fn patch_settings(&self, patch: SettingsPatch) -> AppResult<()> {
        self.request(
            |reply| ControlCommand::PatchSettings { patch, reply },
            "patch_settings",
        )
        .await
    }

    async fn submit_text(&self, request: SubmitTextRequest) -> AppResult<EventId> {
        self.request(
            |reply| ControlCommand::SubmitText { request, reply },
            "submit_text",
        )
        .await
    }

    async fn get_clipboard_record(&self, event_id: EventId) -> AppResult<ClipboardRecord> {
        self.request(
            |reply| ControlCommand::GetClipboardRecord { event_id, reply },
            "get_clipboard_record",
        )
        .await
    }

    async fn list_clipboard_history(
        &self,
        request: ListClipboardHistoryRequest,
    ) -> AppResult<ClipboardHistoryPage> {
        self.request(
            |reply| ControlCommand::ListClipboardHistory { request, reply },
            "list_clipboard_history",
        )
        .await
    }

    async fn adopt_clipboard_record(&self, event_id: EventId) -> AppResult<()> {
        self.request(
            |reply| ControlCommand::AdoptClipboardRecord { event_id, reply },
            "adopt_clipboard_record",
        )
        .await
    }

    async fn rebroadcast_clipboard_record(
        &self,
        request: RebroadcastClipboardRequest,
    ) -> AppResult<()> {
        self.request(
            |reply| ControlCommand::RebroadcastClipboardRecord { request, reply },
            "rebroadcast_clipboard_record",
        )
        .await
    }

    async fn send_files(&self, request: SendFilesRequest) -> AppResult<Vec<TransferId>> {
        self.request(
            |reply| ControlCommand::SendFiles { request, reply },
            "send_files",
        )
        .await
    }

    async fn decide_incoming_transfer(&self, request: IncomingTransferDecision) -> AppResult<()> {
        self.request(
            |reply| ControlCommand::DecideIncomingTransfer { request, reply },
            "decide_incoming_transfer",
        )
        .await
    }

    async fn cancel_transfer(&self, transfer_id: TransferId) -> AppResult<()> {
        self.request(
            |reply| ControlCommand::CancelTransfer { transfer_id, reply },
            "cancel_transfer",
        )
        .await
    }
}
