use super::{EventId, Targets};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalClipboardChangeRequest {
    pub event_id: EventId,
    pub text: String,
    pub targets: Targets,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalClipboardChangeResult {
    pub event_id: EventId,
    pub broadcast_status: BroadcastStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BroadcastStatus {
    NotRequested,
    Sent,
    Dropped(BroadcastDropReason),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BroadcastDropReason {
    NetworkDisabled,
    EngineNotRunning,
    NoEligiblePeer,
    QueueFull,
    QueueClosed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RebroadcastHistoryRequest {
    pub event_id: EventId,
    pub targets: Targets,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteTextRequest {
    pub event_id: EventId,
    pub content: String,
    pub noob_id: String,
    pub device_id: String,
}
