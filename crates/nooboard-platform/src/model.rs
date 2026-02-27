use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipboardEvent {
    pub text: String,
    pub timestamp: SystemTime,
}

impl ClipboardEvent {
    pub fn new(text: String) -> Self {
        Self {
            text,
            timestamp: SystemTime::now(),
        }
    }

    pub fn timestamp_millis(&self) -> u128 {
        self.timestamp
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or(0)
    }
}
