//! Driver-free behavioral checks. Future adapters run these same functions.
mod history;
mod settings;
use nooboard_storage::*;

pub async fn exercise(storage: &Storage) {
    history::exercise(storage).await;
    settings::exercise(storage).await;
}
pub fn record(text: &str, time: i64, source: HistorySource) -> RecordHistory {
    RecordHistory {
        entry: NewHistoryEntry {
            text: text.into(),
            copied_at_ms: time,
            source,
        },
        retention: Retention {
            max_entries: 100,
            oldest_ms: 0,
        },
    }
}
