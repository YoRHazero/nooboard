use std::path::Path;
use std::sync::Arc;

use tokio::sync::{Mutex, RwLock, broadcast};

use crate::AppResult;
use crate::clipboard_runtime::{ClipboardPort, ClipboardRuntime};
use crate::config::AppConfig;
use crate::storage_runtime::StorageRuntime;
use crate::sync_runtime::SyncRuntime;

use super::events::SubscriptionHub;
use super::types::{
    AppEvent, AppSyncStatus, BroadcastConfig, ConnectedPeer, EventId, FileDecisionRequest,
    HistoryCursor, HistoryPage, HistoryRecord, ListHistoryRequest, LocalClipboardChangeRequest,
    LocalClipboardChangeResult, NetworkPatch, RebroadcastHistoryRequest, RemoteTextRequest,
    SendFileRequest, StorageConfigView, StoragePatch, find_recent_record, now_millis_i64,
};

mod clipboard_history;
mod config_patch_network;
mod config_patch_storage;
mod config_transcation;
mod engine;
mod files;
mod subscriptions;

#[allow(async_fn_in_trait)]
pub trait AppService {
    async fn start_engine(&self) -> AppResult<()>;
    async fn stop_engine(&self) -> AppResult<()>;
    async fn restart_engine(&self) -> AppResult<()>;
    async fn sync_status(&self) -> AppResult<AppSyncStatus>;
    async fn connected_peers(&self) -> AppResult<Vec<ConnectedPeer>>;

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

    async fn subscribe_events(&self) -> AppResult<broadcast::Receiver<AppEvent>>;

    async fn apply_network_patch(&self, patch: NetworkPatch) -> AppResult<BroadcastConfig>;
    async fn apply_storage_patch(&self, patch: StoragePatch) -> AppResult<StorageConfigView>;
}

pub struct AppServiceImpl {
    config_path: std::path::PathBuf,
    config: Arc<RwLock<AppConfig>>,
    storage_runtime: Arc<StorageRuntime>,
    clipboard: ClipboardRuntime,
    sync_runtime: Arc<Mutex<SyncRuntime>>,
    config_update_lock: Arc<Mutex<()>>,
    subscriptions: Arc<SubscriptionHub>,
}

impl AppServiceImpl {
    pub fn new(
        config_path: impl AsRef<Path>,
        clipboard: Arc<dyn ClipboardPort>,
    ) -> AppResult<Self> {
        let config_path = config_path.as_ref().to_path_buf();
        let config = AppConfig::load(&config_path)?;
        let storage_runtime = Arc::new(StorageRuntime::new(config.to_storage_config())?);

        Ok(Self {
            config_path,
            config: Arc::new(RwLock::new(config)),
            storage_runtime,
            clipboard: ClipboardRuntime::new(clipboard),
            sync_runtime: Arc::new(Mutex::new(SyncRuntime::new())),
            config_update_lock: Arc::new(Mutex::new(())),
            subscriptions: Arc::new(SubscriptionHub::new()),
        })
    }
}

impl AppService for AppServiceImpl {
    async fn start_engine(&self) -> AppResult<()> {
        self.start_engine_usecase().await
    }

    async fn stop_engine(&self) -> AppResult<()> {
        self.stop_engine_usecase().await
    }

    async fn restart_engine(&self) -> AppResult<()> {
        self.restart_engine_usecase().await
    }

    async fn sync_status(&self) -> AppResult<AppSyncStatus> {
        self.sync_status_usecase().await
    }

    async fn connected_peers(&self) -> AppResult<Vec<ConnectedPeer>> {
        self.connected_peers_usecase().await
    }

    async fn apply_local_clipboard_change(
        &self,
        request: LocalClipboardChangeRequest,
    ) -> AppResult<LocalClipboardChangeResult> {
        self.apply_local_clipboard_change_usecase(request).await
    }

    async fn apply_history_entry_to_clipboard(&self, event_id: EventId) -> AppResult<()> {
        self.apply_history_entry_to_clipboard_usecase(event_id)
            .await
    }

    async fn list_history(&self, request: ListHistoryRequest) -> AppResult<HistoryPage> {
        self.list_history_usecase(request).await
    }

    async fn rebroadcast_history_entry(&self, request: RebroadcastHistoryRequest) -> AppResult<()> {
        self.rebroadcast_history_entry_usecase(request).await
    }

    async fn store_remote_text(&self, request: RemoteTextRequest) -> AppResult<()> {
        self.store_remote_text_usecase(request).await
    }

    async fn write_remote_text_to_clipboard(&self, request: RemoteTextRequest) -> AppResult<()> {
        self.write_remote_text_to_clipboard_usecase(request).await
    }

    async fn send_file(&self, request: SendFileRequest) -> AppResult<()> {
        self.send_file_usecase(request).await
    }

    async fn respond_file_decision(&self, request: FileDecisionRequest) -> AppResult<()> {
        self.respond_file_decision_usecase(request).await
    }

    async fn subscribe_events(&self) -> AppResult<broadcast::Receiver<AppEvent>> {
        self.subscribe_events_usecase().await
    }

    async fn apply_network_patch(&self, patch: NetworkPatch) -> AppResult<BroadcastConfig> {
        self.apply_network_patch_usecase(patch).await
    }

    async fn apply_storage_patch(&self, patch: StoragePatch) -> AppResult<StorageConfigView> {
        self.apply_storage_patch_usecase(patch).await
    }
}
