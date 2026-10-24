use crate::ipc::{
    contracts::*,
    errors::{self, UiError},
};
pub fn id(id: &str) -> Result<i64, UiError> {
    id.parse()
        .ok()
        .filter(|id| *id > 0)
        .ok_or(errors::ui("invalidHistoryAction"))
}
pub async fn query(app: &nooboard_core::App, query: HistoryQuery) -> Result<Reply, UiError> {
    if query.contains.len() > 4096 {
        return Err(errors::ui("invalidHistoryQuery"));
    }
    let local = match query.source {
        HistorySource::All => None,
        HistorySource::Local => Some(true),
        HistorySource::Remote => Some(false),
    };
    let mut rows = app
        .history_filtered(query.contains, local, 33, query.offset)
        .await
        .map_err(errors::core)?;
    let has_more = rows.len() > 32;
    rows.truncate(32);
    Ok(Reply::History(HistoryPage {
        items: rows.into_iter().map(Into::into).collect(),
        has_more,
    }))
}
