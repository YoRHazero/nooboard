//! The public application boundary. Handles never own a service lifetime.
pub use crate::{
    configuration::model::{Mode, PeerSettings, Settings},
    devices::model::{
        LocalNetwork, OnboardingSnapshot, PairingError, PairingFailure, PairingSession,
    },
    error::{Error, Result},
    history::model::HistoryEntry,
    options::Options,
    snapshot::model::{
        ActivityKind, ActivityRecord, AppSnapshot, AppState, ClipboardKind, CurrentClipboard,
        Event, Fault, PeerStatus, Status,
    },
    sync::model::{
        ContentStage, ContentTransfer, Delivery, DeliveryState, Transfer, TransferFailure,
    },
};
pub use nooboard_network::{
    ContentKind, LocalAddress, NearbyDevice, PairingStage, TransferId as MessageId,
};
// Backend selection is a startup concern; business APIs contain no SQL or driver types.
pub use nooboard_storage::{BackendConfig, SqliteOptions};
#[cfg(feature = "diagnostics")]
pub mod diagnostics {
    pub use crate::PeerFixture;
    pub use crate::diagnostic_support::{Session, trust_peer};
}
#[cfg(any(test, feature = "diagnostics"))]
pub use crate::devices::model::PeerFixture;
use crate::{
    configuration::runtime::{self as configuration, Change},
    devices::runtime as devices,
    history::runtime as history,
    runtime::message::request,
    sync::runtime as sync,
};
use tokio::{
    sync::{broadcast, mpsc, watch},
    task::JoinHandle,
};
/// Sole lifetime owner. Drop requests cleanup; shutdown waits for all owned services.
pub struct AppService {
    pub(crate) stop: watch::Sender<bool>,
    pub(crate) task: Option<JoinHandle<Result<()>>>,
}
impl AppService {
    pub async fn start(options: Options) -> Result<(Self, App)> {
        crate::runtime::startup::start(options).await
    }
    pub async fn shutdown(mut self) -> Result<()> {
        self.stop.send_replace(true);
        match self.task.take() {
            Some(task) => task.await.map_err(|_| Error::Internal)?,
            None => Ok(()),
        }
    }
}
impl Drop for AppService {
    fn drop(&mut self) {
        self.stop.send_replace(true);
    }
}
#[derive(Clone)]
pub struct App {
    pub(crate) configuration: configuration::Handle,
    pub(crate) devices: mpsc::Sender<devices::Request>,
    pub(crate) sync: mpsc::Sender<sync::Request>,
    pub(crate) history: mpsc::Sender<history::Request>,
    pub(crate) snapshots: watch::Receiver<AppSnapshot>,
    pub(crate) events: broadcast::Sender<Event>,
    pub(crate) stopped: watch::Receiver<bool>,
    #[cfg(any(test, feature = "diagnostics"))]
    pub(crate) certificate: Vec<u8>,
}
impl App {
    async fn configuration_visible(&self) -> Result<()> {
        let revision = self.configuration.current().revision;
        let mut snapshots = self.snapshots.clone();
        loop {
            if snapshots.borrow_and_update().status.configuration_revision >= revision {
                return Ok(());
            }
            snapshots.changed().await.map_err(|_| Error::Stopped)?;
        }
    }
    pub fn is_running(&self) -> bool {
        !*self.stopped.borrow() && self.snapshot().status.state == AppState::Running
    }
    pub fn snapshot(&self) -> AppSnapshot {
        self.snapshots.borrow().clone()
    }
    pub fn status(&self) -> Status {
        self.snapshot().status
    }
    pub fn subscribe(&self) -> watch::Receiver<AppSnapshot> {
        self.snapshots.clone()
    }
    pub fn subscribe_snapshots(&self) -> watch::Receiver<AppSnapshot> {
        self.subscribe()
    }
    /// Advisory notifications for diagnostics. Recover from lag using the current snapshot.
    pub fn subscribe_events(&self) -> broadcast::Receiver<Event> {
        self.events.subscribe()
    }
    #[cfg(any(test, feature = "diagnostics"))]
    pub fn certificate(&self) -> &[u8] {
        &self.certificate
    }
    pub async fn set_settings(&self, settings: Settings) -> Result<()> {
        self.configuration
            .change(Change::Settings(settings))
            .await?;
        self.configuration_visible().await
    }
    pub async fn select_targets(&self, targets: Vec<String>) -> Result<()> {
        self.configuration.change(Change::Targets(targets)).await?;
        self.configuration_visible().await
    }
    pub async fn refresh_discovery(&self) -> Result<()> {
        request(&self.devices, &self.stopped, |reply| {
            devices::Request::Refresh { reply }
        })
        .await
    }
    pub async fn begin_pairing(&self, address: String, expected: Option<String>) -> Result<()> {
        request(&self.devices, &self.stopped, |reply| {
            devices::Request::Begin {
                address,
                expected,
                reply,
            }
        })
        .await
    }
    pub async fn accept_pairing(&self, id: String) -> Result<()> {
        request(&self.devices, &self.stopped, |reply| {
            devices::Request::Accept { id, reply }
        })
        .await
    }
    pub async fn submit_pairing_code(&self, id: String, code: String) -> Result<()> {
        request(&self.devices, &self.stopped, |reply| {
            devices::Request::Code { id, code, reply }
        })
        .await
    }
    pub async fn dismiss_pairing(&self, id: String) -> Result<()> {
        request(&self.devices, &self.stopped, |reply| {
            devices::Request::Dismiss { id, reply }
        })
        .await
    }
    pub async fn unpair(&self, id: String) -> Result<()> {
        request(&self.devices, &self.stopped, |reply| {
            devices::Request::Unpair { id, reply }
        })
        .await?;
        self.configuration_visible().await
    }
    pub async fn configure_peer(&self, id: String, settings: PeerSettings) -> Result<()> {
        request(&self.devices, &self.stopped, |reply| {
            devices::Request::Configure {
                id,
                settings,
                reply,
            }
        })
        .await?;
        self.configuration_visible().await
    }
    pub async fn send_current(&self) -> Result<MessageId> {
        request(&self.sync, &self.stopped, |reply| sync::Request::Send {
            targets: None,
            reply,
        })
        .await
    }
    pub async fn send_to(&self, targets: Vec<String>) -> Result<MessageId> {
        request(&self.sync, &self.stopped, |reply| sync::Request::Send {
            targets: Some(targets),
            reply,
        })
        .await
    }
    pub async fn send_files(&self, paths: Vec<std::path::PathBuf>) -> Result<()> {
        request(&self.sync, &self.stopped, |reply| sync::Request::Files {
            paths,
            reply,
        })
        .await
    }
    pub async fn cancel_transfer(&self, key: String) -> Result<()> {
        request(&self.sync, &self.stopped, |reply| sync::Request::Cancel {
            key,
            reply,
        })
        .await
    }
    pub async fn copy_received(&self, key: String) -> Result<()> {
        request(&self.sync, &self.stopped, |reply| {
            sync::Request::CopyReceived { key, reply }
        })
        .await
    }
    pub async fn copy_history(&self, id: i64) -> Result<()> {
        request(&self.sync, &self.stopped, |reply| {
            sync::Request::CopyHistory { id, reply }
        })
        .await
    }
    pub async fn history(
        &self,
        contains: String,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<HistoryEntry>> {
        self.history_filtered(contains, None, limit, offset).await
    }
    pub async fn history_filtered(
        &self,
        contains: String,
        local: Option<bool>,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<HistoryEntry>> {
        request(&self.history, &self.stopped, |reply| {
            history::Request::Query {
                contains,
                local,
                limit,
                offset,
                reply,
            }
        })
        .await
    }
    pub async fn delete_history(&self, id: i64) -> Result<()> {
        request(&self.history, &self.stopped, |reply| {
            history::Request::Delete { id, reply }
        })
        .await
    }
    pub async fn clear_history(&self) -> Result<()> {
        request(&self.history, &self.stopped, |reply| {
            history::Request::Clear { reply }
        })
        .await
    }
    #[cfg(any(test, feature = "diagnostics"))]
    pub(crate) async fn trust_peer(&self, fixture: PeerFixture) -> Result<String> {
        request(&self.devices, &self.stopped, |reply| {
            devices::Request::Trust { fixture, reply }
        })
        .await
    }
}
