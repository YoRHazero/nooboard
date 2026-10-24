use crate::contracts::clipboard::{Payload, ServiceStatus, Snapshot};
use std::{future::Future, pin::Pin};
use tokio::sync::watch;

pub(crate) type ClipboardFuture<'a> =
    Pin<Box<dyn Future<Output = crate::contracts::clipboard::Result<Snapshot>> + Send + 'a>>;
pub(crate) trait ClipboardPort: Send + Sync {
    fn subscribe_status(&self) -> Option<watch::Receiver<ServiceStatus>> {
        None
    }
    fn subscribe(&self) -> watch::Receiver<Option<Snapshot>>;
    fn read(&self) -> ClipboardFuture<'_>;
    fn write_content(&self, content: Payload) -> ClipboardFuture<'_>;
}
