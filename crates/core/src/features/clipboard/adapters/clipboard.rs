use crate::features::clipboard::ports::clipboard::{ClipboardFuture, ClipboardPort};
use nooboard_clipboard::{Clipboard, Payload, ServiceStatus, Snapshot};
use tokio::sync::watch;
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
