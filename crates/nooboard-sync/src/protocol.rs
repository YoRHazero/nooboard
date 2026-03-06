use postcard::{from_bytes, to_extend};
use serde::{Deserialize, Serialize};

use crate::error::ProtocolError;

pub const PROTOCOL_VERSION: u16 = 2;

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
        noob_id: String,
        device_id: String,
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
        event_id: String,
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
    to_extend(packet, Vec::new()).map_err(ProtocolError::Serialize)
}

pub fn decode_packet(bytes: &[u8]) -> Result<Packet, ProtocolError> {
    from_bytes(bytes).map_err(ProtocolError::Deserialize)
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

    #[test]
    fn packet_round_trip_via_postcard() {
        let packet = Packet::Data(DataPacket::FileChunk {
            transfer_id: 7,
            seq: 3,
            data: vec![1, 2, 3, 4],
        });

        let encoded = encode_packet(&packet).expect("packet should serialize");
        let decoded = decode_packet(&encoded).expect("packet should deserialize");

        assert_eq!(decoded, packet);
    }
}
