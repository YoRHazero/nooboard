use std::sync::{Arc, atomic::AtomicBool};
use std::thread::JoinHandle;
use std::time::Duration;

use nooboard_platform::{ClipboardBackend, ClipboardEventSender};

use crate::error::{CoreError, CoreResult};

pub trait ClipboardPort: Send + Sync {
    fn read_text(&self) -> CoreResult<Option<String>>;
    fn write_text(&self, text: &str) -> CoreResult<()>;
    fn watch_changes(
        &self,
        sender: ClipboardEventSender,
        shutdown: Arc<AtomicBool>,
        interval: Duration,
    ) -> CoreResult<JoinHandle<()>>;
}

impl<T> ClipboardPort for T
where
    T: ClipboardBackend,
{
    fn read_text(&self) -> CoreResult<Option<String>> {
        ClipboardBackend::read_text(self).map_err(|error| CoreError::Clipboard(error.to_string()))
    }

    fn write_text(&self, text: &str) -> CoreResult<()> {
        ClipboardBackend::write_text(self, text)
            .map_err(|error| CoreError::Clipboard(error.to_string()))
    }

    fn watch_changes(
        &self,
        sender: ClipboardEventSender,
        shutdown: Arc<AtomicBool>,
        interval: Duration,
    ) -> CoreResult<JoinHandle<()>> {
        ClipboardBackend::watch_changes(self, sender, shutdown, interval)
            .map_err(|error| CoreError::Clipboard(error.to_string()))
    }
}
