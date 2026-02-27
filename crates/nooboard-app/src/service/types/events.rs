use std::net::SocketAddr;

use super::{EventId, NodeId, TransferUpdate};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventStream {
    Sync,
    Transfer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncEvent {
    TextReceived {
        event_id: EventId,
        content: String,
        device_id: String,
    },
    FileDecisionRequired {
        peer_node_id: NodeId,
        transfer_id: u32,
        file_name: String,
        file_size: u64,
        total_chunks: u32,
    },
    ConnectionError {
        peer_node_id: Option<NodeId>,
        addr: Option<SocketAddr>,
        error: String,
    },
    BridgeLagged {
        stream: EventStream,
        dropped: u64,
    },
    BridgeMappingFailed {
        error: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppEvent {
    Sync(SyncEvent),
    Transfer(TransferUpdate),
}
