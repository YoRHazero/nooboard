//! Thin IPC entry points; core coordinates the three backend libraries.
pub mod devices;
pub mod history;
pub mod onboarding;
pub mod settings;
use crate::wire;
use crate::{errors, host::Host};
use tauri::State;
use tauri::{AppHandle, ipc::Channel};
#[tauri::command]
pub async fn desktop_connect(
    handle: AppHandle,
    host: State<'_, Host>,
    on_frame: Channel<wire::Frame>,
) -> Result<wire::Connection, crate::errors::UiError> {
    host.connect(&handle, on_frame).await
}
#[tauri::command]
pub async fn desktop_send(host: State<'_, Host>) -> Result<(), crate::errors::UiError> {
    let guard = host.running.read().await;
    guard
        .as_ref()
        .ok_or(crate::errors::ui("backendNotConnected"))?
        .app()
        .send_current()
        .await
        .map(|_| ())
        .map_err(errors::core)
}
#[tauri::command]
pub async fn desktop_probe(
    host: State<'_, Host>,
    action: String,
) -> Result<(), crate::errors::UiError> {
    #[cfg(all(debug_assertions, feature = "diagnostics", target_os = "macos"))]
    if let Some(crate::host::Running::Diagnostic(diagnostic)) = host.running.write().await.as_mut()
    {
        return diagnostic.action(&action).await;
    }
    let _ = (host, action);
    Err(crate::errors::ui("diagnosticsDisabled"))
}
