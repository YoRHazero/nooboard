use std::net::SocketAddr;
use std::path::PathBuf;

use tokio::sync::watch;

use super::{EventId, LocalIdentity, TransfersState};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppState {
    pub revision: u64,
    pub identity: LocalIdentity,
    pub local_connection: LocalConnectionInfo,
    pub sync: SyncState,
    pub peers: PeersState,
    pub clipboard: ClipboardState,
    pub transfers: TransfersState,
    pub settings: SettingsState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncState {
    pub desired: SyncDesiredState,
    pub actual: SyncActualStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SyncDesiredState {
    Running,
    #[default]
    Stopped,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncActualStatus {
    Disabled,
    Starting,
    Running,
    Stopped,
    Error(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PeersState {
    pub connected: Vec<ConnectedPeer>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectedPeer {
    pub noob_id: super::NoobId,
    pub device_id: String,
    pub addresses: Vec<SocketAddr>,
    pub transport: PeerTransport,
    pub latency_ms: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerTransport {
    Mdns,
    Manual,
    Mixed,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ClipboardState {
    pub latest_committed_event_id: Option<EventId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LocalConnectionInfo {
    pub device_endpoint: Option<SocketAddr>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsState {
    pub connection_identity: ConnectionIdentitySettings,
    pub network: NetworkSettings,
    pub storage: StorageSettings,
    pub clipboard: ClipboardSettings,
    pub transfers: TransferSettings,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionIdentitySettings {
    pub device_id: String,
    pub token: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkSettings {
    pub listen_port: u16,
    pub network_enabled: bool,
    pub mdns_enabled: bool,
    pub manual_peers: Vec<SocketAddr>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageSettings {
    pub db_root: PathBuf,
    pub history_window_days: u32,
    pub dedup_window_days: u32,
    pub max_text_bytes: usize,
    pub gc_batch_size: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipboardSettings {
    pub local_capture_enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferSettings {
    pub download_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingsPatch {
    ConnectionIdentity(ConnectionIdentitySettingsPatch),
    Network(NetworkSettingsPatch),
    Storage(StorageSettingsPatch),
    Clipboard(ClipboardSettingsPatch),
    Transfers(TransferSettingsPatch),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionIdentitySettingsPatch {
    Replace(ConnectionIdentitySettings),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetworkSettingsPatch {
    SetListenPort(u16),
    SetNetworkEnabled(bool),
    SetMdnsEnabled(bool),
    SetManualPeers(Vec<SocketAddr>),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StorageSettingsPatch {
    pub db_root: Option<PathBuf>,
    pub history_window_days: Option<u32>,
    pub dedup_window_days: Option<u32>,
    pub max_text_bytes: Option<usize>,
    pub gc_batch_size: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClipboardSettingsPatch {
    SetLocalCaptureEnabled(bool),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransferSettingsPatch {
    SetDownloadDir(PathBuf),
}

pub type StateRecvError = watch::error::RecvError;

pub struct StateSubscription {
    latest: AppState,
    receiver: watch::Receiver<AppState>,
}

impl StateSubscription {
    pub(crate) fn new(receiver: watch::Receiver<AppState>) -> Self {
        let latest = receiver.borrow().clone();
        Self { latest, receiver }
    }

    pub async fn recv(&mut self) -> Result<AppState, StateRecvError> {
        self.receiver.changed().await?;
        self.latest = self.receiver.borrow().clone();
        Ok(self.latest.clone())
    }

    pub fn latest(&self) -> &AppState {
        &self.latest
    }
}
