use super::ports::{ExitInteraction, Interaction};
use crate::{
    features::{preferences::application::PreferenceService, window::application::WindowService},
    localization,
};
use std::sync::Arc;
use tauri::{AppHandle, Manager};
pub struct NativeExit {
    pub handle: AppHandle,
    pub window: Arc<WindowService>,
    pub preferences: Arc<PreferenceService>,
}
impl NativeExit {
    fn text(&self, key: &str) -> String {
        localization::text(&self.preferences.snapshot().resolved_language, key)
    }
}
impl ExitInteraction for NativeExit {
    fn confirm(&self, count: usize) -> Interaction<'_, bool> {
        Box::pin(async move {
            self.window.show(None);
            let quit = self.text("menuQuit");
            let mut dialog = rfd::AsyncMessageDialog::new()
                .set_title(self.text("trayQuitTitle"))
                .set_description(
                    self.text("trayQuitMessage")
                        .replace("{{count}}", &count.to_string()),
                )
                .set_buttons(rfd::MessageButtons::OkCancelCustom(
                    quit.clone(),
                    self.text("trayKeepRunning"),
                ));
            if let Some(window) = self.handle.get_webview_window("main") {
                dialog = dialog.set_parent(&window);
            }
            let result = dialog.show().await;
            result == rfd::MessageDialogResult::Ok
                || result == rfd::MessageDialogResult::Custom(quit)
        })
    }
    fn failed(&self) -> Interaction<'_, ()> {
        Box::pin(async move {
            self.window.show(None);
            rfd::AsyncMessageDialog::new()
                .set_title("Nooboard")
                .set_description(self.text("quitFailedMessage"))
                .set_buttons(rfd::MessageButtons::Ok)
                .show()
                .await;
        })
    }
    fn exit(&self) {
        self.handle.exit(0);
    }
}
