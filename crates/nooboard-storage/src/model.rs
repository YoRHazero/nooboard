#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventState {
    Active,
    Tombstone,
}

impl EventState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Tombstone => "tombstone",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryRecord {
    pub event_id: [u8; 16],
    pub origin_device_id: String,
    pub created_at_ms: i64,
    pub applied_at_ms: i64,
    pub content: String,
}

impl HistoryRecord {
    pub fn event_id_hex(&self) -> String {
        self.event_id
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }

    pub fn cursor(&self) -> HistoryCursor {
        HistoryCursor {
            created_at_ms: self.created_at_ms,
            event_id: self.event_id,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HistoryCursor {
    pub created_at_ms: i64,
    pub event_id: [u8; 16],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutboxMessage {
    pub id: i64,
    pub event_id: [u8; 16],
    pub content: String,
    pub target_key: String,
    pub targets: Option<Vec<String>>,
    pub attempt_count: u32,
    pub next_attempt_at_ms: i64,
}
