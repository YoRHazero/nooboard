use super::{EventId, NoobId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardRecordSource {
    LocalCapture,
    RemoteSync,
    UserSubmit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipboardRecord {
    pub event_id: EventId,
    pub source: ClipboardRecordSource,
    pub origin_noob_id: NoobId,
    pub origin_device_id: String,
    pub created_at_ms: i64,
    pub applied_at_ms: i64,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListClipboardHistoryRequest {
    pub limit: usize,
    pub cursor: Option<ClipboardHistoryCursor>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipboardHistoryPage {
    pub records: Vec<ClipboardRecord>,
    pub next_cursor: Option<ClipboardHistoryCursor>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipboardHistoryCursor {
    pub created_at_ms: i64,
    pub event_id: EventId,
}

impl ClipboardHistoryCursor {
    pub(crate) fn to_storage_cursor(&self) -> nooboard_storage::HistoryCursor {
        nooboard_storage::HistoryCursor {
            created_at_ms: self.created_at_ms,
            event_id: *self.event_id.as_uuid().as_bytes(),
        }
    }

    pub(crate) fn from_storage(value: &nooboard_storage::HistoryRecord) -> Self {
        Self {
            created_at_ms: value.created_at_ms,
            event_id: EventId::from(uuid::Uuid::from_bytes(value.event_id)),
        }
    }
}

impl ClipboardRecord {
    pub(crate) fn from_storage(
        value: nooboard_storage::HistoryRecord,
        source: ClipboardRecordSource,
    ) -> Self {
        Self {
            event_id: EventId::from(uuid::Uuid::from_bytes(value.event_id)),
            source,
            origin_noob_id: NoobId::new(value.origin_noob_id),
            origin_device_id: value.origin_device_id,
            created_at_ms: value.created_at_ms,
            applied_at_ms: value.applied_at_ms,
            content: value.content,
        }
    }
}
