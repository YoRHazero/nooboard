use super::ports::WindowPort;
use tauri::{AppHandle, Manager};
pub struct NativeWindow(pub AppHandle);
impl WindowPort for NativeWindow {
    fn show(&self) -> bool {
        let handle = &self.0;
        let window = handle.get_webview_window("main").or_else(|| {
            let config = handle
                .config()
                .app
                .windows
                .iter()
                .find(|c| c.label == "main")?;
            tauri::WebviewWindowBuilder::from_config(handle, config)
                .ok()?
                .build()
                .ok()
        });
        if let Some(window) = window {
            let _ = window.unminimize();
            if window.show().is_ok() {
                let _ = window.set_focus();
                return true;
            }
        }
        false
    }
    fn hide(&self) -> bool {
        self.0
            .get_webview_window("main")
            .is_some_and(|w| w.hide().is_ok())
    }
}
