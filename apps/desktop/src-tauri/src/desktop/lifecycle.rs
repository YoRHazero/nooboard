//! Explicit quit is separate from hiding a window. Repeated quit requests share one shutdown.
use super::{Desktop, TRAY_SUPPORTED, show};
use crate::host::Host;
use std::sync::atomic::{AtomicU8, Ordering};
use tauri::{AppHandle, Manager};

const IDLE: u8 = 0;
const ASKING: u8 = 1;
const CLOSING: u8 = 2;
const FINISHED: u8 = 3;
#[derive(Default)]
pub struct ExitControl(AtomicU8);
impl ExitControl {
    fn ask(&self) -> bool {
        self.0
            .compare_exchange(IDLE, ASKING, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }
    fn cancel(&self) {
        let _ = self
            .0
            .compare_exchange(ASKING, IDLE, Ordering::SeqCst, Ordering::SeqCst);
    }
    fn begin(&self) -> bool {
        self.0
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |v| {
                (v < CLOSING).then_some(CLOSING)
            })
            .is_ok()
    }
    pub fn exiting(&self) -> bool {
        self.0.load(Ordering::SeqCst) >= CLOSING
    }
    pub fn finished(&self) -> bool {
        self.0.load(Ordering::SeqCst) == FINISHED
    }
}
pub fn should_hide(supported: bool, available: bool, enabled: bool, exiting: bool) -> bool {
    supported && available && enabled && !exiting
}

pub fn request_quit(handle: &AppHandle) {
    if !handle.state::<Desktop>().exit.ask() {
        return;
    }
    let handle = handle.clone();
    tauri::async_runtime::spawn(async move {
        let count = {
            let host = handle.state::<Host>();
            let running = host.running.read().await;
            running
                .as_ref()
                .map(|running| {
                    let snapshot = running.app().snapshot();
                    snapshot
                        .content_transfers
                        .iter()
                        .filter(|t| t.stage.pending())
                        .count()
                        + snapshot
                            .status
                            .transfers
                            .iter()
                            .filter(|t| t.pending())
                            .count()
                })
                .unwrap_or(0)
        };
        let desktop = handle.state::<Desktop>();
        if count > 0 && TRAY_SUPPORTED && !desktop.exit.exiting() {
            show(&handle, None);
            let quit = desktop.text("menuQuit");
            let mut dialog = rfd::AsyncMessageDialog::new()
                .set_title(desktop.text("trayQuitTitle"))
                .set_description(
                    desktop
                        .text("trayQuitMessage")
                        .replace("{{count}}", &count.to_string()),
                )
                .set_buttons(rfd::MessageButtons::OkCancelCustom(
                    quit.clone(),
                    desktop.text("trayKeepRunning"),
                ));
            if let Some(window) = handle.get_webview_window("main") {
                dialog = dialog.set_parent(&window);
            }
            let result = dialog.show().await;
            if result != rfd::MessageDialogResult::Ok
                && result != rfd::MessageDialogResult::Custom(quit)
            {
                desktop.exit.cancel();
                return;
            }
        }
        shutdown(&handle);
    });
}
/// ExitRequested (including OS session termination) never waits for a user dialog.
pub fn shutdown(handle: &AppHandle) {
    if !handle.state::<Desktop>().exit.begin() {
        return;
    }
    handle.state::<Host>().closing.store(true, Ordering::SeqCst);
    let handle = handle.clone();
    tauri::async_runtime::spawn(async move {
        let _ = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            handle.state::<Host>().shutdown(),
        )
        .await;
        handle
            .state::<Desktop>()
            .exit
            .0
            .store(FINISHED, Ordering::SeqCst);
        handle.exit(0);
    });
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn repeated_quits_and_late_dialog_results_cannot_bypass_shutdown() {
        let exit = ExitControl::default();
        assert!(exit.ask());
        assert!(!exit.ask());
        assert!(exit.begin());
        exit.cancel();
        assert!(exit.exiting());
        assert!(!exit.finished());
        assert!(!exit.begin());
        assert!(!exit.ask());
    }
    #[test]
    fn unsupported_or_failed_tray_never_hides_the_window() {
        assert!(should_hide(true, true, true, false));
        for (supported, available, enabled, exiting) in [
            (false, true, true, false),
            (true, false, true, false),
            (true, true, false, false),
            (true, true, true, true),
        ] {
            assert!(!should_hide(supported, available, enabled, exiting));
        }
    }
}
