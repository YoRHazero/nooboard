use crate::Settings;
pub(crate) fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(i64::MAX as u128) as i64
}
pub(crate) fn retention(settings: &Settings, now: i64) -> nooboard_storage::Retention {
    nooboard_storage::Retention {
        max_entries: settings.max_history_entries,
        oldest_ms: now.saturating_sub(i64::from(settings.history_days) * 86_400_000),
    }
}
