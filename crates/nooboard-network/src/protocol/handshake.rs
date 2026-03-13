use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionIntent {
    LanSync,
    DirectConnect,
}

pub const PROTOCOL_VERSION: u16 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HandshakePacket {
    Hello {
        protocol_version: u16,
        noob_id: String,
        device_id: String,
        intent: ConnectionIntent,
    },
    Challenge {
        nonce: String,
    },
    AuthResponse {
        hash: String,
    },
    AuthAccepted,
    AuthRejected {
        reason: String,
    },
}
