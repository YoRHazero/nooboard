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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryRecordSource {
    LocalCapture,
    RemoteSync,
    UserSubmit,
}

impl HistoryRecordSource {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LocalCapture => "local_capture",
            Self::RemoteSync => "remote_sync",
            Self::UserSubmit => "user_submit",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "local_capture" => Some(Self::LocalCapture),
            "remote_sync" => Some(Self::RemoteSync),
            "user_submit" => Some(Self::UserSubmit),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryRecord {
    pub event_id: [u8; 16],
    pub origin_noob_id: String,
    pub origin_device_id: String,
    pub created_at_ms: i64,
    pub applied_at_ms: i64,
    pub content: String,
    pub source: HistoryRecordSource,
}

impl HistoryRecord {
    pub fn event_id_hex(&self) -> String {
        self.event_id
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }

    pub fn anchor(&self) -> HistoryAnchor {
        HistoryAnchor {
            created_at_ms: self.created_at_ms,
            event_id: self.event_id,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryDirection {
    Older,
    Newer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HistoryAnchor {
    pub created_at_ms: i64,
    pub event_id: [u8; 16],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListHistoryRequest {
    pub limit: usize,
    pub direction: HistoryDirection,
    pub anchor: Option<HistoryAnchor>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryPage {
    pub records: Vec<HistoryRecord>,
    pub has_more: bool,
    pub next_anchor: Option<HistoryAnchor>,
}
