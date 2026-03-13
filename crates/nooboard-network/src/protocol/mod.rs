use postcard::{from_bytes, to_extend};
use serde::{Deserialize, Serialize};

mod control;
mod data;
mod handshake;

pub use control::{ControlPacket, DirectRequestStatus};
pub use data::DataPacket;
pub use handshake::{ConnectionIntent, HandshakePacket, PROTOCOL_VERSION};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Packet {
    Handshake(HandshakePacket),
    Control(ControlPacket),
    Ping { timestamp: u64 },
    Pong { timestamp: u64 },
    Data(DataPacket),
}

pub(crate) fn encode_packet(packet: &Packet) -> Result<Vec<u8>, crate::errors::ProtocolError> {
    to_extend(packet, Vec::new()).map_err(crate::errors::ProtocolError::Serialize)
}

pub(crate) fn decode_packet(bytes: &[u8]) -> Result<Packet, crate::errors::ProtocolError> {
    from_bytes(bytes).map_err(crate::errors::ProtocolError::Deserialize)
}

pub(crate) fn require_handshake(
    packet: Packet,
) -> Result<HandshakePacket, crate::errors::ProtocolError> {
    match packet {
        Packet::Handshake(handshake) => Ok(handshake),
        _ => Err(crate::errors::ProtocolError::HandshakeRequired),
    }
}
