use super::{
    adapters::core::{Backend, CoreConfig},
    runtime::Runtime,
};
use crate::{
    features::{
        file_selection::{adapters::NativeFilePicker, application::FileSelection},
        lifecycle::{adapters::NativeExit, application::Lifecycle, domain::ExitControl},
        preferences::{
            adapters::{JsonPreferences, NativeLocale},
            application::PreferenceService,
        },
        tray::{
            adapters::{NativeTray, SUPPORTED},
            application::{TrayService, presentation},
            domain::{Action, Summary},
        },
        window::{adapters::NativeWindow, application::WindowService},
    },
    ipc,
};
use std::sync::Arc;
use tauri::{AppHandle, Manager};
pub fn build(handle: &AppHandle) -> Arc<Runtime> {
    let runtime_handle = tauri::async_runtime::handle();
    let _entered = runtime_handle.inner().enter();
    let diagnostic = cfg!(all(
        debug_assertions,
        feature = "diagnostics",
        target_os = "macos"
    )) && std::env::var("NOOBOARD_DIAGNOSTIC").as_deref() == Ok("1");
    let directory = handle.path().app_data_dir().ok();
    let download_history = directory
        .as_ref()
        .filter(|_| !diagnostic)
        .map(|p| p.join("download-directory.json"));
    let preferences = Arc::new(PreferenceService::load(
        Arc::new(JsonPreferences {
            path: if diagnostic {
                None
            } else {
                directory
                    .as_ref()
                    .map(|p| p.join("desktop-preferences.json"))
            },
            unavailable: !diagnostic && directory.is_none(),
        }),
        Arc::new(NativeLocale),
        SUPPORTED,
    ));
    let window = Arc::new(WindowService::new(Arc::new(NativeWindow(handle.clone()))));
    let control = Arc::new(ExitControl::default());
    let backend = Arc::new(Backend::new(
        CoreConfig {
            directory,
            downloads: handle.path().download_dir().ok(),
            diagnostic,
        },
        control.clone(),
    ));
    let lifecycle = Arc::new(Lifecycle::new(
        control,
        backend.clone(),
        Arc::new(NativeExit {
            handle: handle.clone(),
            window: window.clone(),
            preferences: preferences.clone(),
        }),
        SUPPORTED,
    ));
    let (actions, mut inbox) = tokio::sync::mpsc::unbounded_channel();
    let tray = Arc::new(TrayService::new(
        Arc::new(NativeTray::create(
            handle,
            presentation(
                &Summary::Starting,
                &preferences.snapshot().resolved_language,
            ),
            actions,
        )),
        preferences.snapshot().resolved_language,
    ));
    let runtime = Arc::new(Runtime::new(
        backend.clone(),
        window,
        preferences,
        tray,
        lifecycle.clone(),
        FileSelection::new(Arc::new(NativeFilePicker::new(download_history))),
        (!cfg!(debug_assertions) && !diagnostic).then(|| {
            Arc::new(crate::features::update::application::Updater::new(
                Arc::new(crate::features::update::adapters::NativeUpdateSource(
                    handle.clone(),
                )),
                Arc::new(super::adapters::update::NativeInstall {
                    lifecycle,
                    handle: handle.clone(),
                }),
            ))
        }),
    ));
    let weak = Arc::downgrade(&runtime);
    runtime.track(tokio::spawn(async move {
        while let Some(action) = inbox.recv().await {
            let Some(runtime) = weak.upgrade() else {
                return;
            };
            match action {
                Action::Show(page) => {
                    if !runtime.lifecycle.control.exiting() {
                        runtime.window.show(page);
                    }
                }
                Action::Quit => {
                    let lifecycle = runtime.lifecycle.clone();
                    tokio::spawn(async move {
                        lifecycle.request_quit().await;
                    });
                }
            }
        }
    }));
    runtime.track(tokio::spawn(async move {
        let _ = backend.ensure_started().await;
    }));
    runtime
}
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_single_instance::init(|handle, _, _| {
            super::events::show(handle, None)
        }))
        .on_menu_event(|handle, event| {
            if event.id.as_ref() == "app-quit" {
                super::events::request_quit(handle);
            }
        })
        .setup(|app| {
            app.manage(build(app.handle()));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ipc::entry::connect,
            ipc::entry::disconnect,
            ipc::entry::request
        ])
        .build(tauri::generate_context!())
        .expect("could not create nooboard window")
        .run(super::events::dispatch);
}
