#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod app;
#[cfg(all(debug_assertions, feature = "diagnostics", target_os = "macos"))]
mod diagnostics;
mod error;
mod features;
mod ipc;
mod localization;
fn main() {
    app::bootstrap::run();
}
