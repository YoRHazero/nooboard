//! Discovery and one-time-code pairing commands; all trust changes belong to core.
use crate::{errors, host::Host};
use tauri::State;
#[tauri::command]
pub async fn desktop_discover(host: State<'_, Host>) -> Result<(), crate::errors::UiError> {
    host.app()
        .await?
        .refresh_discovery()
        .await
        .map_err(errors::core)
}
#[tauri::command]
pub async fn desktop_begin_pairing(
    host: State<'_, Host>,
    address: String,
    expected: Option<String>,
) -> Result<(), crate::errors::UiError> {
    host.app()
        .await?
        .begin_pairing(address, expected)
        .await
        .map_err(errors::core)
}
#[tauri::command]
pub async fn desktop_accept_pairing(
    host: State<'_, Host>,
    id: String,
) -> Result<(), crate::errors::UiError> {
    host.app()
        .await?
        .accept_pairing(id)
        .await
        .map_err(errors::core)
}
#[tauri::command]
pub async fn desktop_pairing_code(
    host: State<'_, Host>,
    id: String,
    code: String,
) -> Result<(), crate::errors::UiError> {
    host.app()
        .await?
        .submit_pairing_code(id, code)
        .await
        .map_err(errors::core)
}
#[tauri::command]
pub async fn desktop_dismiss_pairing(
    host: State<'_, Host>,
    id: String,
) -> Result<(), crate::errors::UiError> {
    host.app()
        .await?
        .dismiss_pairing(id)
        .await
        .map_err(errors::core)
}
