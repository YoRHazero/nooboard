use std::sync::{Arc, atomic::AtomicBool};
use std::thread::JoinHandle;
use std::time::Duration;

use tokio::sync::mpsc;

use crate::{ClipboardEvent, NooboardError};

pub type ClipboardEventSender = mpsc::Sender<ClipboardEvent>;

pub const DEFAULT_WATCH_INTERVAL: Duration = Duration::from_millis(250);

pub trait ClipboardBackend: Send + Sync {
    fn read_text(&self) -> Result<Option<String>, NooboardError>;

    fn write_text(&self, text: &str) -> Result<(), NooboardError>;

    fn watch_changes(
        &self,
        sender: ClipboardEventSender,
        shutdown: Arc<AtomicBool>,
        interval: Duration,
    ) -> Result<JoinHandle<()>, NooboardError>;
}
