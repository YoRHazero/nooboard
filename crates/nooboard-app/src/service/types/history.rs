use nooboard_storage::HistoryCursor as StorageHistoryCursor;

use super::EventId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryRecord {
    pub event_id: EventId,
    pub origin_noob_id: String,
    pub origin_device_id: String,
    pub created_at_ms: i64,
    pub applied_at_ms: i64,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryCursor {
    pub created_at_ms: i64,
    pub event_id: EventId,
}

impl HistoryCursor {
    pub(crate) fn to_storage_cursor(&self) -> StorageHistoryCursor {
        StorageHistoryCursor {
            created_at_ms: self.created_at_ms,
            event_id: *self.event_id.as_uuid().as_bytes(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryPage {
    pub records: Vec<HistoryRecord>,
    pub next_cursor: Option<HistoryCursor>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListHistoryRequest {
    pub limit: usize,
    pub cursor: Option<HistoryCursor>,
}
