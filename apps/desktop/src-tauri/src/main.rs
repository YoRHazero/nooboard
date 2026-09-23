#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod desktop;
#[cfg(all(debug_assertions, feature = "diagnostics", target_os = "macos"))]
mod diagnostics;
mod host;
mod ipc;
use tauri::Manager;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_single_instance::init(|handle, _, _| {
            desktop::show(handle, None);
        }))
        .manage(host::Host::default())
        .on_menu_event(|handle, event| {
            if event.id.as_ref() == "app-quit" {
                desktop::lifecycle::request_quit(handle);
            }
        })
        .setup(|app| {
            app.manage(desktop::Desktop::load(app.handle()));
            desktop::tray::init(app.handle());
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                // The UI may connect later or reconnect without owning the backend lifetime.
                let _ = handle.state::<host::Host>().ensure_started(&handle).await;
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ipc::connect,
            ipc::disconnect,
            ipc::request
        ])
        .build(tauri::generate_context!())
        .expect("could not create nooboard window")
        .run(|handle, event| match event {
            tauri::RunEvent::WindowEvent {
                label,
                event: tauri::WindowEvent::CloseRequested { api, .. },
                ..
            } if label == "main" => {
                let desktop = handle.state::<desktop::Desktop>();
                let state = desktop.snapshot();
                if desktop::lifecycle::should_hide(
                    state.tray_supported,
                    state.tray_available,
                    state.close_to_tray,
                    desktop.exit.exiting(),
                ) && let Some(window) = handle.get_webview_window("main")
                    && window.hide().is_ok()
                {
                    api.prevent_close();
                    desktop.visible(false);
                }
            }
            tauri::RunEvent::WindowEvent {
                label,
                event: tauri::WindowEvent::Focused(true),
                ..
            } if label == "main" => {
                handle.state::<desktop::Desktop>().visible(true);
            }
            #[cfg(target_os = "macos")]
            tauri::RunEvent::Reopen { .. } => desktop::show(handle, None),
            tauri::RunEvent::ExitRequested { api, .. }
                if !handle.state::<desktop::Desktop>().exit.finished() =>
            {
                api.prevent_exit();
                desktop::lifecycle::shutdown(handle);
            }
            _ => {}
        });
}
