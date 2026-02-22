use serde::{Deserialize, Serialize};

use crate::SyncError;

pub const PROTOCOL_VERSION: u16 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HelloMessage {
    pub version: u16,
    pub device_id: String,
    pub token: String,
}

impl HelloMessage {
    pub fn new(device_id: impl Into<String>, token: impl Into<String>) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            device_id: device_id.into(),
            token: token.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SyncEvent {
    pub version: u16,
    pub origin_device_id: String,
    pub origin_seq: u64,
    pub captured_at: i64,
    pub content: String,
}

impl SyncEvent {
    pub fn new(
        origin_device_id: impl Into<String>,
        origin_seq: u64,
        captured_at: i64,
        content: impl Into<String>,
    ) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            origin_device_id: origin_device_id.into(),
            origin_seq,
            captured_at,
            content: content.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum WireMessage {
    Hello(HelloMessage),
    Event(SyncEvent),
}

pub fn encode_message(message: &WireMessage) -> Result<String, SyncError> {
    Ok(serde_json::to_string(message)?)
}

pub fn decode_message(raw: &str) -> Result<WireMessage, SyncError> {
    Ok(serde_json::from_str(raw)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_decode_hello_roundtrip() -> Result<(), SyncError> {
        let hello = WireMessage::Hello(HelloMessage::new("dev-a", "token"));
        let encoded = encode_message(&hello)?;
        let decoded = decode_message(&encoded)?;
        assert_eq!(decoded, hello);
        Ok(())
    }

    #[test]
    fn encode_decode_event_roundtrip() -> Result<(), SyncError> {
        let event = WireMessage::Event(SyncEvent::new("dev-a", 7, 1_700_000_000, "hello"));
        let encoded = encode_message(&event)?;
        let decoded = decode_message(&encoded)?;
        assert_eq!(decoded, event);
        Ok(())
    }
}
