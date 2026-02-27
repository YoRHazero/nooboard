use std::net::SocketAddr;
use std::path::PathBuf;

use super::NodeId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetworkPatch {
    SetMdnsEnabled(bool),
    SetNetworkEnabled(bool),
    AddManualPeer(SocketAddr),
    RemoveManualPeer(SocketAddr),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StoragePatch {
    pub db_root: Option<PathBuf>,
    pub retain_old_versions: Option<usize>,
    pub history_window_days: Option<u32>,
    pub dedup_window_days: Option<u32>,
    pub gc_every_inserts: Option<u32>,
    pub gc_batch_size: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppPatch {
    Network(NetworkPatch),
    Storage(StoragePatch),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageConfigView {
    pub db_root: PathBuf,
    pub retain_old_versions: usize,
    pub history_window_days: u32,
    pub dedup_window_days: u32,
    pub gc_every_inserts: u32,
    pub gc_batch_size: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppSyncStatus {
    Disabled,
    Starting,
    Running,
    Stopped,
    Error(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerConnectionState {
    Connected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectedPeer {
    pub peer_node_id: NodeId,
    pub peer_device_id: String,
    pub addr: SocketAddr,
    pub outbound: bool,
    pub connected_at_ms: u64,
    pub state: PeerConnectionState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SyncDesiredState {
    Running,
    #[default]
    Stopped,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppServiceSnapshot {
    pub desired_state: SyncDesiredState,
    pub actual_sync_status: AppSyncStatus,
    pub connected_peers: Vec<ConnectedPeer>,
    pub network_enabled: bool,
    pub mdns_enabled: bool,
    pub manual_peers: Vec<SocketAddr>,
    pub storage: StorageConfigView,
}
