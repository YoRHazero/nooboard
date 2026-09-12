//! Native clipboard access on a dedicated platform thread, without arboard.
mod model;
#[cfg(target_os = "macos")]
#[path = "platform/macos.rs"]
mod platform;
#[cfg(target_os = "windows")]
#[path = "platform/windows.rs"]
mod platform;
#[cfg(target_os = "linux")]
#[path = "platform/linux/mod.rs"]
mod platform;
#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
#[path = "platform/unsupported.rs"]
mod platform;
mod worker;
pub use model::{Content, Error, Origin, Result, Snapshot};
pub use worker::Clipboard;
