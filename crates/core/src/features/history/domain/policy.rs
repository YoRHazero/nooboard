use crate::Settings;
pub(crate) fn retention(settings: &Settings, now: i64) -> crate::contracts::storage::Retention {
    crate::contracts::storage::Retention {
        max_entries: settings.max_history_entries,
        oldest_ms: now.saturating_sub(i64::from(settings.history_days) * 86_400_000),
    }
}
