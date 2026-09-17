#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod commands;
#[cfg(all(debug_assertions, feature = "diagnostics", target_os = "macos"))]
mod diagnostics;
mod errors;
mod host;
mod wire;
use std::sync::atomic::Ordering;
use tauri::Manager;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_single_instance::init(|handle, _, _| {
            if let Some(window) = handle.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .manage(host::Host::default())
        .invoke_handler(tauri::generate_handler![
            commands::onboarding::desktop_discover,
            commands::onboarding::desktop_begin_pairing,
            commands::onboarding::desktop_accept_pairing,
            commands::onboarding::desktop_pairing_code,
            commands::onboarding::desktop_dismiss_pairing,
            commands::desktop_connect,
            commands::settings::desktop_settings,
            commands::devices::desktop_peer_settings,
            commands::devices::desktop_select_targets,
            commands::desktop_send,
            commands::transfers::desktop_select_files,
            commands::transfers::desktop_receive_directory,
            commands::transfers::desktop_transfer_action,
            commands::devices::desktop_unpair,
            commands::history::desktop_history,
            commands::history::desktop_history_action,
            commands::desktop_probe
        ])
        .build(tauri::generate_context!())
        .expect("could not create nooboard window")
        .run(|handle, event| {
            if let tauri::RunEvent::ExitRequested { api, .. } = event {
                let host = handle.state::<host::Host>();
                if !host.closing.swap(true, Ordering::SeqCst) {
                    api.prevent_exit();
                    let handle = handle.clone();
                    tauri::async_runtime::spawn(async move {
                        let _ = tokio::time::timeout(
                            std::time::Duration::from_secs(5),
                            handle.state::<host::Host>().shutdown(),
                        )
                        .await;
                        handle.exit(0);
                    });
                }
            }
        });
}
