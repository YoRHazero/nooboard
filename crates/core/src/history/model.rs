use serde::Serialize;
#[derive(Clone, Serialize, PartialEq, Eq)]
pub struct HistoryEntry {
    pub id: i64,
    pub text: String,
    pub source: String,
    pub copied_at_ms: i64,
}
impl std::fmt::Debug for HistoryEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HistoryEntry")
            .field("id", &self.id)
            .field("bytes", &self.text.len())
            .field("source", &self.source)
            .field("copied_at_ms", &self.copied_at_ms)
            .finish()
    }
}

impl From<nooboard_storage::HistoryEntry> for HistoryEntry {
    fn from(row: nooboard_storage::HistoryEntry) -> Self {
        Self {
            id: row.id.value(),
            text: row.entry.text,
            source: match row.entry.source {
                nooboard_storage::HistorySource::Local => "local".into(),
                nooboard_storage::HistorySource::Remote(peer) => peer,
            },
            copied_at_ms: row.entry.copied_at_ms,
        }
    }
}
#[derive(Clone, Default)]
pub(crate) struct HistoryState {
    pub revision: u64,
    pub error: Option<String>,
}
