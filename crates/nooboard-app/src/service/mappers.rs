use nooboard_sync::{
    ConnectedPeerInfo as SyncConnectedPeerInfo, PeerConnectionState as SyncPeerConnectionState,
    SyncEvent as SyncEngineEvent, SyncStatus as SyncEngineStatus,
    TransferDirection as SyncTransferDirection, TransferState as SyncTransferState,
    TransferUpdate as SyncTransferUpdate,
};

use super::types::{
    AppEvent, AppSyncStatus, ConnectedPeer, EventId, HistoryCursor, HistoryRecord, NoobId,
    PeerConnectionState, SyncEvent, TransferDirection, TransferState, TransferUpdate,
};

impl From<SyncEngineStatus> for AppSyncStatus {
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

impl From<SyncPeerConnectionState> for PeerConnectionState {
    fn from(value: SyncPeerConnectionState) -> Self {
        match value {
            SyncPeerConnectionState::Connected => Self::Connected,
        }
    }
}

impl From<SyncConnectedPeerInfo> for ConnectedPeer {
    fn from(value: SyncConnectedPeerInfo) -> Self {
        Self {
            peer_noob_id: NoobId::new(value.peer_noob_id),
            peer_device_id: value.peer_device_id,
            addr: value.addr,
            outbound: value.outbound,
            connected_at_ms: value.connected_at_ms,
            state: value.state.into(),
        }
    }
}

impl From<SyncTransferDirection> for TransferDirection {
    fn from(value: SyncTransferDirection) -> Self {
        match value {
            SyncTransferDirection::Incoming => Self::Incoming,
            SyncTransferDirection::Outgoing => Self::Outgoing,
        }
    }
}

impl From<SyncTransferState> for TransferState {
    fn from(value: SyncTransferState) -> Self {
        match value {
            SyncTransferState::Started {
                file_name,
                total_bytes,
            } => Self::Started {
                file_name,
                total_bytes,
            },
            SyncTransferState::Progress {
                done_bytes,
                total_bytes,
                bps,
                eta_ms,
            } => Self::Progress {
                done_bytes,
                total_bytes,
                bps,
                eta_ms,
            },
            SyncTransferState::Finished { path } => Self::Finished { path },
            SyncTransferState::Failed { reason } => Self::Failed { reason },
            SyncTransferState::Cancelled { reason } => Self::Cancelled { reason },
        }
    }
}

impl From<SyncTransferUpdate> for TransferUpdate {
    fn from(value: SyncTransferUpdate) -> Self {
        Self {
            transfer_id: value.transfer_id,
            peer_noob_id: NoobId::new(value.peer_noob_id),
            direction: value.direction.into(),
            state: value.state.into(),
        }
    }
}

impl TryFrom<SyncEngineEvent> for SyncEvent {
    type Error = crate::AppError;

    fn try_from(value: SyncEngineEvent) -> Result<Self, Self::Error> {
        match value {
            SyncEngineEvent::TextReceived {
                event_id,
                content,
                noob_id,
                device_id,
            } => Ok(Self::TextReceived {
                event_id: EventId::try_from(event_id.as_str())?,
                content,
                noob_id: NoobId::new(noob_id),
                device_id,
            }),
            SyncEngineEvent::FileDecisionRequired {
                peer_noob_id,
                transfer_id,
                file_name,
                file_size,
                total_chunks,
            } => Ok(Self::FileDecisionRequired {
                peer_noob_id: NoobId::new(peer_noob_id),
                transfer_id,
                file_name,
                file_size,
                total_chunks,
            }),
            SyncEngineEvent::ConnectionError {
                peer_noob_id,
                addr,
                error,
            } => Ok(Self::ConnectionError {
                peer_noob_id: peer_noob_id.map(NoobId::new),
                addr,
                error,
            }),
        }
    }
}

impl From<SyncTransferUpdate> for AppEvent {
    fn from(value: SyncTransferUpdate) -> Self {
        Self::Transfer(value.into())
    }
}

impl TryFrom<SyncEngineEvent> for AppEvent {
    type Error = crate::AppError;

    fn try_from(value: SyncEngineEvent) -> Result<Self, Self::Error> {
        Ok(Self::Sync(value.try_into()?))
    }
}

impl From<nooboard_storage::HistoryRecord> for HistoryRecord {
    fn from(value: nooboard_storage::HistoryRecord) -> Self {
        Self {
            event_id: EventId::from(uuid::Uuid::from_bytes(value.event_id)),
            origin_noob_id: value.origin_noob_id,
            origin_device_id: value.origin_device_id,
            created_at_ms: value.created_at_ms,
            applied_at_ms: value.applied_at_ms,
            content: value.content,
        }
    }
}

impl From<&nooboard_storage::HistoryRecord> for HistoryCursor {
    fn from(value: &nooboard_storage::HistoryRecord) -> Self {
        Self {
            created_at_ms: value.created_at_ms,
            event_id: EventId::from(uuid::Uuid::from_bytes(value.event_id)),
        }
    }
}
