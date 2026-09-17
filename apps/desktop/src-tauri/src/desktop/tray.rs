//! The sole platform boundary for tray support. Ubuntu has no tray resources.
use tauri::AppHandle;

#[cfg(any(target_os = "macos", target_os = "windows"))]
mod supported {
    use super::*;
    use crate::desktop::{Desktop, Page, lifecycle, show};
    use std::sync::Mutex;
    use tauri::{
        Manager,
        menu::{Menu, MenuItem, PredefinedMenuItem},
        tray::{TrayIcon, TrayIconBuilder},
    };

    #[derive(Default, Clone, PartialEq, Eq)]
    enum Summary {
        #[default]
        Starting,
        Running {
            paused: bool,
            connected: usize,
            transfers: usize,
        },
        Failed,
    }
    pub struct Tray {
        _icon: TrayIcon,
        status: MenuItem<tauri::Wry>,
        open: MenuItem<tauri::Wry>,
        transfers: MenuItem<tauri::Wry>,
        settings: MenuItem<tauri::Wry>,
        quit: MenuItem<tauri::Wry>,
        summary: Mutex<Summary>,
    }
    pub fn init(handle: &AppHandle) -> tauri::Result<()> {
        let desktop = handle.state::<Desktop>();
        let item = |id: &str, text: &str, enabled| {
            MenuItem::with_id(handle, id, desktop.text(text), enabled, None::<&str>)
        };
        let status = item("tray-status", "starting", false)?;
        let open = item("tray-open", "trayOpen", true)?;
        let transfers = item("tray-transfers", "transfers", true)?;
        let settings = item("tray-settings", "settings", true)?;
        let quit = item("tray-quit", "menuQuit", true)?;
        let separator = PredefinedMenuItem::separator(handle)?;
        let menu = Menu::with_items(
            handle,
            &[&status, &separator, &open, &transfers, &settings, &quit],
        )?;
        let mut builder = TrayIconBuilder::with_id("nooboard")
            .menu(&menu)
            .icon_as_template(cfg!(target_os = "macos"))
            .show_menu_on_left_click(cfg!(target_os = "macos"))
            .tooltip("Nooboard")
            .on_menu_event(|handle, event| match event.id.as_ref() {
                "tray-open" => show(handle, None),
                "tray-transfers" => show(handle, Some(Page::Transfers)),
                "tray-settings" => show(handle, Some(Page::Settings)),
                "tray-quit" => lifecycle::request_quit(handle),
                _ => {}
            });
        if let Some(icon) = handle.default_window_icon() {
            builder = builder.icon(icon.clone());
        }
        #[cfg(target_os = "windows")]
        {
            builder = builder.on_tray_icon_event(|tray, event| {
                if matches!(
                    event,
                    tauri::tray::TrayIconEvent::Click {
                        button: tauri::tray::MouseButton::Left,
                        button_state: tauri::tray::MouseButtonState::Up,
                        ..
                    }
                ) {
                    show(tray.app_handle(), None);
                }
            });
        }
        let icon = builder.build(handle)?;
        handle.manage(Tray {
            _icon: icon,
            status,
            open,
            transfers,
            settings,
            quit,
            summary: Mutex::new(Summary::Starting),
        });
        desktop.change(|s| s.tray_available = true);
        Ok(())
    }
    pub fn refresh(handle: &AppHandle) {
        let Some(tray) = handle.try_state::<Tray>() else {
            return;
        };
        let desktop = handle.state::<Desktop>();
        let summary = tray.summary.lock().unwrap().clone();
        let text = match summary {
            Summary::Starting => desktop.text("starting"),
            Summary::Failed => desktop.text("startFailed"),
            Summary::Running { paused: true, .. } => desktop.text("trayPaused"),
            Summary::Running {
                connected,
                transfers,
                ..
            } => desktop
                .text("trayStatus")
                .replace("{{connected}}", &connected.to_string())
                .replace("{{transfers}}", &transfers.to_string()),
        };
        let _ = tray.status.set_text(text);
        for (item, key) in [
            (&tray.open, "trayOpen"),
            (&tray.transfers, "transfers"),
            (&tray.settings, "settings"),
            (&tray.quit, "menuQuit"),
        ] {
            let _ = item.set_text(desktop.text(key));
        }
    }
    pub fn update(handle: &AppHandle, snapshot: Option<&nooboard_core::AppSnapshot>) {
        let Some(tray) = handle.try_state::<Tray>() else {
            return;
        };
        let next = snapshot
            .map(|s| Summary::Running {
                paused: s.status.settings.paused,
                connected: s.status.peers.iter().filter(|p| p.online).count(),
                transfers: s
                    .content_transfers
                    .iter()
                    .filter(|t| t.stage.pending())
                    .count()
                    + s.status.transfers.iter().filter(|t| t.pending()).count(),
            })
            .unwrap_or(Summary::Failed);
        let changed = {
            let mut current = tray.summary.lock().unwrap();
            if *current == next {
                false
            } else {
                *current = next;
                true
            }
        };
        if changed {
            refresh(handle);
        }
    }
}

pub fn init(handle: &AppHandle) {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    if let Err(error) = supported::init(handle) {
        eprintln!("Tray initialization failed: {error}");
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let _ = handle;
}
pub fn refresh(handle: &AppHandle) {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    supported::refresh(handle);
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let _ = handle;
}
pub fn update(handle: &AppHandle, snapshot: Option<&nooboard_core::AppSnapshot>) {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    supported::update(handle, snapshot);
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let _ = (handle, snapshot);
}
