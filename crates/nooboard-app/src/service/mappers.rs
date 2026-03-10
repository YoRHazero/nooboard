use nooboard_sync::{
    ConnectedPeerInfo as SyncConnectedPeerInfo, SyncEvent as SyncEngineEvent,
    SyncStatus as SyncEngineStatus, TransferDirection as SyncTransferDirection,
    TransferUpdate as SyncTransferUpdate,
};

use super::types::{
    AppEvent, ClipboardHistoryCursor, ClipboardRecord, ClipboardRecordSource, ConnectedPeer,
    EventId, NoobId, PeerTransport, SyncActualStatus, TransferDirection, TransferId,
};

impl From<SyncEngineStatus> for SyncActualStatus {
    fn from(value: SyncEngineStatus) -> Self {
        match value {
            SyncEngineStatus::Disabled => Self::Disabled,
            SyncEngineStatus::Starting => Self::Starting,
            SyncEngineStatus::Running => Self::Running,
            SyncEngineStatus::Stopped => Self::Stopped,
            SyncEngineStatus::Error(message) => Self::Error(message),
        }
    }
}

impl From<SyncTransferDirection> for TransferDirection {
    fn from(value: SyncTransferDirection) -> Self {
        match value {
            SyncTransferDirection::Incoming => Self::Download,
            SyncTransferDirection::Outgoing => Self::Upload,
        }
    }
}

impl From<&nooboard_storage::HistoryRecord> for ClipboardHistoryCursor {
    fn from(value: &nooboard_storage::HistoryRecord) -> Self {
        Self {
            created_at_ms: value.created_at_ms,
            event_id: EventId::from(uuid::Uuid::from_bytes(value.event_id)),
        }
    }
}

impl ClipboardRecord {
    pub(crate) fn from_storage(
        value: nooboard_storage::HistoryRecord,
        source: ClipboardRecordSource,
    ) -> Self {
        Self {
            event_id: EventId::from(uuid::Uuid::from_bytes(value.event_id)),
            source,
            origin_noob_id: NoobId::new(value.origin_noob_id),
            origin_device_id: value.origin_device_id,
            created_at_ms: value.created_at_ms,
            applied_at_ms: value.applied_at_ms,
            content: value.content,
        }
    }
}

pub(crate) fn map_connected_peer(
    value: SyncConnectedPeerInfo,
    transport: PeerTransport,
) -> ConnectedPeer {
    ConnectedPeer {
        noob_id: NoobId::new(value.peer_noob_id),
        device_id: value.peer_device_id,
        addresses: vec![value.addr],
        transport,
        latency_ms: None,
    }
}

pub(crate) fn map_transfer_id(update: &SyncTransferUpdate) -> TransferId {
    TransferId::new(NoobId::new(update.peer_noob_id.clone()), update.transfer_id)
}

pub(crate) fn map_sync_connection_error(value: SyncEngineEvent) -> Option<AppEvent> {
    match value {
        SyncEngineEvent::ConnectionError {
            peer_noob_id,
            addr,
            error,
        } => Some(AppEvent::PeerConnectionError {
            peer_noob_id: peer_noob_id.map(NoobId::new),
            addr,
            error,
        }),
        _ => None,
    }
}
