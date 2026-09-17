//! Native selection and task commands; all data handling remains in core.
use crate::{errors, host::Host};
use tauri::State;

#[tauri::command]
pub async fn desktop_select_files(host: State<'_, Host>) -> Result<(), errors::UiError> {
    let Some(files) = rfd::AsyncFileDialog::new().pick_files().await else {
        return Ok(());
    };
    let paths = files.iter().map(|f| f.path().to_owned()).collect();
    let guard = host.running.read().await;
    guard
        .as_ref()
        .ok_or(errors::ui("backendNotConnected"))?
        .app()
        .send_files(paths)
        .await
        .map_err(errors::core)
}
#[tauri::command]
pub async fn desktop_receive_directory(host: State<'_, Host>) -> Result<(), errors::UiError> {
    let Some(directory) = rfd::AsyncFileDialog::new().pick_folder().await else {
        return Ok(());
    };
    let _mutation = host.mutations.lock().await;
    let guard = host.running.read().await;
    let app = guard
        .as_ref()
        .ok_or(errors::ui("backendNotConnected"))?
        .app();
    let mut settings = app.status().settings;
    settings.receive_directory = Some(directory.path().to_owned());
    app.set_settings(settings).await.map_err(errors::core)
}
#[tauri::command]
pub async fn desktop_transfer_action(
    host: State<'_, Host>,
    key: String,
    action: String,
) -> Result<(), errors::UiError> {
    let guard = host.running.read().await;
    let app = guard
        .as_ref()
        .ok_or(errors::ui("backendNotConnected"))?
        .app();
    match action.as_str() {
        "cancel" => app.cancel_transfer(key).await,
        "copy" => app.copy_received(key).await,
        _ => return Err(errors::ui("configuration")),
    }
    .map_err(errors::core)
}
