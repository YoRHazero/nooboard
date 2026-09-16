use crate::{HistoryEntry, Result, Settings, ports::Store};
use std::time::{SystemTime, UNIX_EPOCH};
pub(crate) fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(i64::MAX as u128) as i64
}
fn cutoff(settings: &Settings, now: i64) -> i64 {
    now.saturating_sub(i64::from(settings.history_days) * 86_400_000)
}
pub(crate) async fn record(
    store: &Store,
    settings: &Settings,
    text: String,
    source: String,
) -> Result<()> {
    if !settings.history {
        return Ok(());
    }
    let now = now_ms();
    let oldest = cutoff(settings, now);
    let limit = settings.max_history_entries;
    store
        .run(move |db| db.record_text(&text, &source, now, limit, oldest))
        .await?;
    Ok(())
}
pub(crate) async fn prune(store: &Store, settings: &Settings) -> Result<bool> {
    let oldest = cutoff(settings, now_ms());
    let limit = settings.max_history_entries;
    store
        .run(move |db| db.prune_history_changed(limit, oldest))
        .await
}
pub(crate) async fn query(
    store: &Store,
    settings: &Settings,
    contains: String,
    local: Option<bool>,
    limit: u32,
    offset: u32,
) -> Result<Vec<HistoryEntry>> {
    prune(store, settings).await?;
    let rows = store
        .run(move |db| {
            db.history_filtered(
                nooboard_storage::HistoryQuery {
                    contains: &contains,
                    limit,
                    offset,
                },
                local,
            )
        })
        .await?;
    Ok(rows.into_iter().map(Into::into).collect())
}
