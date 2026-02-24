use bincode::{deserialize, serialize};
use serde::{Deserialize, Serialize};

use crate::error::ProtocolError;

pub const PROTOCOL_VERSION: u16 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Packet {
    Handshake(HandshakePacket),
    Ping { timestamp: u64 },
    Pong { timestamp: u64 },
    Data(DataPacket),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum HandshakePacket {
    Hello {
        protocol_version: u16,
        node_id: String,
    },
    Challenge {
        nonce: String,
    },
    AuthResponse {
        hash: String,
    },
    AuthResult {
        ok: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DataPacket {
    ClipboardText {
        id: String,
        content: String,
    },
    FileStart {
        transfer_id: u32,
        file_name: String,
        file_size: u64,
        total_chunks: u32,
    },
    FileDecision {
        transfer_id: u32,
        accept: bool,
        reason: Option<String>,
    },
    FileChunk {
        transfer_id: u32,
        seq: u32,
        #[serde(with = "serde_bytes")]
        data: Vec<u8>,
    },
    FileEnd {
        transfer_id: u32,
        checksum: String,
    },
    FileCancel {
        transfer_id: u32,
    },
}

pub fn encode_packet(packet: &Packet) -> Result<Vec<u8>, ProtocolError> {
    serialize(packet).map_err(ProtocolError::Serialize)
}

pub fn decode_packet(bytes: &[u8]) -> Result<Packet, ProtocolError> {
    deserialize(bytes).map_err(ProtocolError::Deserialize)
}

pub fn require_handshake(packet: Packet) -> Result<HandshakePacket, ProtocolError> {
    match packet {
        Packet::Handshake(handshake) => Ok(handshake),
        _ => Err(ProtocolError::HandshakeRequired),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unauthenticated_gate_rejects_non_handshake_packet() {
        let packet = Packet::Ping { timestamp: 1 };
        assert!(matches!(
            require_handshake(packet),
            Err(ProtocolError::HandshakeRequired)
        ));
    }
}
