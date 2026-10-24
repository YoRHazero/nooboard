use super::{
    domain::{Action, Presentation, State},
    ports::TrayPort,
};
use tauri::AppHandle;
use tokio::sync::mpsc;
pub const SUPPORTED: bool = cfg!(any(target_os = "macos", target_os = "windows"));
#[cfg(any(target_os = "macos", target_os = "windows"))]
mod supported {
    use super::*;
    use crate::features::window::domain::Page;
    use tauri::{
        menu::{Menu, MenuItem, PredefinedMenuItem},
        tray::{TrayIcon, TrayIconBuilder},
    };
    pub struct Tray {
        _icon: TrayIcon,
        status: MenuItem<tauri::Wry>,
        open: MenuItem<tauri::Wry>,
        transfers: MenuItem<tauri::Wry>,
        settings: MenuItem<tauri::Wry>,
        quit: MenuItem<tauri::Wry>,
    }
    impl Tray {
        pub fn create(
            handle: &AppHandle,
            view: Presentation,
            actions: mpsc::UnboundedSender<Action>,
        ) -> tauri::Result<Self> {
            let item = |id: &str, text: String, enabled| {
                MenuItem::with_id(handle, id, text, enabled, None::<&str>)
            };
            let status = item("tray-status", view.status, false)?;
            let open = item("tray-open", view.open, true)?;
            let transfers = item("tray-transfers", view.transfers, true)?;
            let settings = item("tray-settings", view.settings, true)?;
            let quit = item("tray-quit", view.quit, true)?;
            let separator = PredefinedMenuItem::separator(handle)?;
            let menu = Menu::with_items(
                handle,
                &[&status, &separator, &open, &transfers, &settings, &quit],
            )?;
            let clicked = actions.clone();
            let mut builder = TrayIconBuilder::with_id("nooboard")
                .menu(&menu)
                .icon_as_template(cfg!(target_os = "macos"))
                .show_menu_on_left_click(cfg!(target_os = "macos"))
                .tooltip("Nooboard")
                .on_menu_event(move |_, event| {
                    let action = match event.id.as_ref() {
                        "tray-open" => Action::Show(None),
                        "tray-transfers" => Action::Show(Some(Page::Transfers)),
                        "tray-settings" => Action::Show(Some(Page::Settings)),
                        "tray-quit" => Action::Quit,
                        _ => return,
                    };
                    let _ = clicked.send(action);
                });
            if let Some(icon) = handle.default_window_icon() {
                builder = builder.icon(icon.clone());
            }
            #[cfg(target_os = "windows")]
            {
                builder = builder.on_tray_icon_event(move |_, event| {
                    if matches!(
                        event,
                        tauri::tray::TrayIconEvent::Click {
                            button: tauri::tray::MouseButton::Left,
                            button_state: tauri::tray::MouseButtonState::Up,
                            ..
                        }
                    ) {
                        let _ = actions.send(Action::Show(None));
                    }
                });
            }
            let icon = builder.build(handle)?;
            Ok(Self {
                _icon: icon,
                status,
                open,
                transfers,
                settings,
                quit,
            })
        }
        pub fn render(&self, view: Presentation) {
            let _ = self.status.set_text(view.status);
            let _ = self.open.set_text(view.open);
            let _ = self.transfers.set_text(view.transfers);
            let _ = self.settings.set_text(view.settings);
            let _ = self.quit.set_text(view.quit);
        }
    }
}
pub struct NativeTray {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    tray: Option<supported::Tray>,
}
impl NativeTray {
    pub fn create(
        handle: &AppHandle,
        view: Presentation,
        actions: mpsc::UnboundedSender<Action>,
    ) -> Self {
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        {
            let tray = supported::Tray::create(handle, view, actions)
                .map_err(|error| eprintln!("Tray initialization failed: {error}"))
                .ok();
            Self { tray }
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            let _ = (handle, view, actions);
            Self {}
        }
    }
}
impl TrayPort for NativeTray {
    fn state(&self) -> State {
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        {
            State {
                supported: SUPPORTED,
                available: self.tray.is_some(),
            }
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            State {
                supported: SUPPORTED,
                available: false,
            }
        }
    }
    fn render(&self, view: Presentation) {
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        if let Some(tray) = &self.tray {
            tray.render(view);
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        let _ = view;
    }
}
