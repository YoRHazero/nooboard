use crate::ipc::contracts::HistoryItem;
impl From<nooboard_core::HistoryEntry> for HistoryItem {
    fn from(row: nooboard_core::HistoryEntry) -> Self {
        Self {
            id: row.id.to_string(),
            text: row.text,
            source: row.source,
            copied_at: row.copied_at_ms,
        }
    }
}
