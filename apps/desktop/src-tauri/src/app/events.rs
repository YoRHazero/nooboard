//! Native event ingress: delegates intent to the composed application objects.
use super::runtime::Runtime;
use crate::features::{lifecycle::domain::should_hide, window::domain::Page};
use std::sync::Arc;
use tauri::{AppHandle, Manager};
pub fn show(handle: &AppHandle, page: Option<Page>) {
    if let Some(runtime) = handle.try_state::<Arc<Runtime>>()
        && !runtime.lifecycle.control.exiting()
    {
        runtime.window.show(page);
    }
}
pub fn request_quit(handle: &AppHandle) {
    if let Some(runtime) = handle.try_state::<Arc<Runtime>>() {
        let lifecycle = runtime.lifecycle.clone();
        tauri::async_runtime::spawn(async move {
            lifecycle.request_quit().await;
        });
    }
}
pub fn dispatch(handle: &AppHandle, event: tauri::RunEvent) {
    let Some(runtime) = handle.try_state::<Arc<Runtime>>() else {
        return;
    };
    match event {
        tauri::RunEvent::WindowEvent {
            label,
            event: tauri::WindowEvent::CloseRequested { api, .. },
            ..
        } if label == "main" => {
            let tray = runtime.tray.snapshot();
            let preferences = runtime.preferences.snapshot();
            if should_hide(
                tray.supported,
                tray.available,
                preferences.close_to_tray,
                runtime.lifecycle.control.exiting(),
            ) && runtime.window.hide()
            {
                api.prevent_close();
            }
        }
        tauri::RunEvent::WindowEvent {
            label,
            event: tauri::WindowEvent::Focused(true),
            ..
        } if label == "main" => runtime.window.visible(true),
        #[cfg(target_os = "macos")]
        tauri::RunEvent::Reopen { .. } => show(handle, None),
        tauri::RunEvent::ExitRequested { api, .. } if !runtime.lifecycle.control.finished() => {
            api.prevent_exit();
            let lifecycle = runtime.lifecycle.clone();
            tauri::async_runtime::spawn(async move {
                lifecycle.shutdown().await;
            });
        }
        _ => {}
    }
}
