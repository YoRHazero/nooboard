use serde::{Deserialize, Serialize};

use crate::DirectRequestId;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DirectRequestStatus {
    PendingApproval,
    Approved,
    Rejected,
    Expired,
    AlreadyConnected,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ControlPacket {
    DirectRequestStatus {
        request_id: DirectRequestId,
        status: DirectRequestStatus,
        expires_at_ms: Option<u64>,
        reason: Option<String>,
    },
    Disconnect {
        reason: Option<String>,
    },
}
