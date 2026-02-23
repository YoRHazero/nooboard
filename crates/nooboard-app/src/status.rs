use std::net::SocketAddr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncState {
    Stopped,
    Starting,
    Running,
    Stopping,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncStatus {
    pub state: SyncState,
    pub listen: Option<SocketAddr>,
    pub connected_peers: usize,
    pub last_error: Option<String>,
    pub last_event_at: Option<i64>,
}

impl Default for SyncStatus {
    fn default() -> Self {
        Self {
            state: SyncState::Stopped,
            listen: None,
            connected_peers: 0,
            last_error: None,
            last_event_at: None,
        }
    }
}
