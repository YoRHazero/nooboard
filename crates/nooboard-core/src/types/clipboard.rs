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
    pub direction: ClipboardHistoryDirection,
    pub anchor: Option<ClipboardHistoryAnchor>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipboardHistoryPage {
    pub records: Vec<ClipboardRecord>,
    pub has_more: bool,
    pub next_anchor: Option<ClipboardHistoryAnchor>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClipboardHistoryDirection {
    Older,
    Newer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipboardHistoryAnchor {
    pub created_at_ms: i64,
    pub event_id: EventId,
}

impl ClipboardHistoryAnchor {
    pub(crate) fn to_storage_anchor(&self) -> nooboard_storage::HistoryAnchor {
        nooboard_storage::HistoryAnchor {
            created_at_ms: self.created_at_ms,
            event_id: *self.event_id.as_uuid().as_bytes(),
        }
    }
}

impl ClipboardHistoryDirection {
    pub(crate) fn to_storage_direction(&self) -> nooboard_storage::HistoryDirection {
        match self {
            Self::Older => nooboard_storage::HistoryDirection::Older,
            Self::Newer => nooboard_storage::HistoryDirection::Newer,
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
