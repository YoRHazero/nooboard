//! Native clipboard access on a dedicated platform thread, without arboard.
#[cfg(any(target_os = "macos", target_os = "linux", test))]
mod file_urls;
mod image_data;
mod model;
pub use image_data::{ImageData, ImageEncoding, MAX_IMAGE_BYTES};
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
#[cfg(test)]
mod native_tests;
