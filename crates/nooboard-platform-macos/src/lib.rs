mod observer;
mod pasteboard;

use std::sync::{Arc, atomic::AtomicBool};
use std::thread::JoinHandle;
use std::time::Duration;

use nooboard_core::NooboardError;
use nooboard_platform::{ClipboardBackend, ClipboardEventSender};

pub struct MacOsClipboardBackend;

impl MacOsClipboardBackend {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MacOsClipboardBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl ClipboardBackend for MacOsClipboardBackend {
    fn read_text(&self) -> Result<Option<String>, NooboardError> {
        pasteboard::read_text_from_pasteboard()
    }

    fn write_text(&self, text: &str) -> Result<(), NooboardError> {
        pasteboard::write_text_to_pasteboard(text)
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
