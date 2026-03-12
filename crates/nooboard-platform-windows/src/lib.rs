//! Windows clipboard backend for nooboard.
//!
//! This crate keeps Win32 clipboard details out of `nooboard-app` and mirrors the
//! existing macOS backend shape:
//! - `read_text` reads `CF_UNICODETEXT`
//! - `write_text` writes `CF_UNICODETEXT` via a Win32 clipboard owner window
//! - `watch_changes` polls `GetClipboardSequenceNumber`, matching the current
//!   macOS change-count polling model

mod clipboard;
mod encoding;
mod observer;

use std::sync::{Arc, atomic::AtomicBool};
use std::thread::JoinHandle;
use std::time::Duration;

use nooboard_platform::{ClipboardBackend, ClipboardEventSender, NooboardError};

pub struct WindowsClipboardBackend;

impl WindowsClipboardBackend {
    pub fn new() -> Self {
        Self
    }
}

impl Default for WindowsClipboardBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl ClipboardBackend for WindowsClipboardBackend {
    fn read_text(&self) -> Result<Option<String>, NooboardError> {
        clipboard::read_text_from_clipboard()
    }

    fn write_text(&self, text: &str) -> Result<(), NooboardError> {
        clipboard::write_text_to_clipboard(text)
    }

    fn watch_changes(
        &self,
        sender: ClipboardEventSender,
        shutdown: Arc<AtomicBool>,
        interval: Duration,
    ) -> Result<JoinHandle<()>, NooboardError> {
        observer::spawn_observer(sender, shutdown, interval)
    }
}
