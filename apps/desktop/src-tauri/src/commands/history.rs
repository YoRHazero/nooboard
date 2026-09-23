//! Paged text history and explicit history operations.
use crate::wire;
use crate::{errors, host::Host};
use tauri::State;
#[tauri::command]
pub async fn desktop_history(
    host: State<'_, Host>,
    contains: String,
    source: String,
    offset: u32,
) -> Result<wire::HistoryPage, crate::errors::UiError> {
    if contains.len() > 4096 {
        return Err(crate::errors::ui("invalidHistoryQuery"));
    }
    let local = match source.as_str() {
        "all" => None,
        "local" => Some(true),
        "remote" => Some(false),
        _ => return Err(crate::errors::ui("invalidHistoryQuery")),
    };
    let mut rows = host
        .app()
        .await?
        .history_filtered(contains, local, 33, offset)
        .await
        .map_err(errors::core)?;
    let has_more = rows.len() > 32;
    rows.truncate(32);
    Ok(wire::HistoryPage {
        items: rows.into_iter().map(Into::into).collect(),
        has_more,
    })
}
#[tauri::command]
pub async fn desktop_history_action(
    host: State<'_, Host>,
    action: String,
    id: Option<String>,
) -> Result<(), crate::errors::UiError> {
    let app = host.app().await?;
    if action == "clear" {
        return app.clear_history().await.map_err(errors::core);
    }
    let id = id
        .and_then(|id| id.parse::<i64>().ok())
        .filter(|id| *id > 0)
        .ok_or(crate::errors::ui("invalidHistoryAction"))?;
    match action.as_str() {
        "copy" => app.copy_history(id).await,
        "delete" => app.delete_history(id).await,
        _ => return Err(crate::errors::ui("invalidHistoryAction")),
    }
    .map_err(errors::core)
}
