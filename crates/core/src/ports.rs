use nooboard_clipboard::{Clipboard, Payload, ServiceStatus, Snapshot};
use std::{future::Future, pin::Pin};
use tokio::sync::watch;

pub(crate) type ClipboardFuture<'a> =
    Pin<Box<dyn Future<Output = nooboard_clipboard::Result<Snapshot>> + Send + 'a>>;
pub(crate) trait ClipboardPort: Send + Sync {
    fn subscribe_status(&self) -> Option<watch::Receiver<ServiceStatus>> {
        None
    }
    fn subscribe(&self) -> watch::Receiver<Option<Snapshot>>;
    fn read(&self) -> ClipboardFuture<'_>;
    fn write_content(&self, content: Payload) -> ClipboardFuture<'_>;
}
impl ClipboardPort for Clipboard {
    fn subscribe_status(&self) -> Option<watch::Receiver<ServiceStatus>> {
        Some(self.subscribe_status())
    }
    fn subscribe(&self) -> watch::Receiver<Option<Snapshot>> {
        self.subscribe()
    }
    fn read(&self) -> ClipboardFuture<'_> {
        Box::pin(self.read())
    }
    fn write_content(&self, content: Payload) -> ClipboardFuture<'_> {
        Box::pin(self.write(content))
    }
}
