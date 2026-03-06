use super::{EventId, Targets};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IngestTextRequest {
    pub event_id: EventId,
    pub content: String,
    pub origin_noob_id: super::NoobId,
    pub origin_device_id: String,
    pub source: TextSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextSource {
    LocalWatch,
    LocalManual,
    RemoteSync,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RebroadcastEventRequest {
    pub event_id: EventId,
    pub targets: Targets,
}
