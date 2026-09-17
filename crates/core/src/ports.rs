use crate::{Error, Result};
use nooboard_clipboard::{Clipboard, Snapshot};
use nooboard_storage::Database;
use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};
use tokio::sync::watch;

pub(crate) type ClipboardFuture<'a> =
    Pin<Box<dyn Future<Output = nooboard_clipboard::Result<Snapshot>> + Send + 'a>>;
pub(crate) trait ClipboardPort: Send + Sync {
    fn subscribe(&self) -> watch::Receiver<nooboard_clipboard::Result<Snapshot>>;
    fn read(&self) -> ClipboardFuture<'_>;
    fn write(&self, text: String) -> ClipboardFuture<'_>;
    fn write_content(&self, content: nooboard_clipboard::Content) -> ClipboardFuture<'_>;
}
impl ClipboardPort for Clipboard {
    fn subscribe(&self) -> watch::Receiver<nooboard_clipboard::Result<Snapshot>> {
        self.subscribe()
    }
    fn read(&self) -> ClipboardFuture<'_> {
        Box::pin(self.read())
    }
    fn write(&self, text: String) -> ClipboardFuture<'_> {
        Box::pin(self.write_text(text))
    }
    fn write_content(&self, content: nooboard_clipboard::Content) -> ClipboardFuture<'_> {
        Box::pin(self.write_content(content))
    }
}

#[derive(Clone)]
pub(crate) struct Store(Arc<Mutex<Database>>);
impl Store {
    pub fn new(database: Database) -> Self {
        Self(Arc::new(Mutex::new(database)))
    }
    pub async fn run<T: Send + 'static>(
        &self,
        f: impl FnOnce(&mut Database) -> nooboard_storage::Result<T> + Send + 'static,
    ) -> Result<T> {
        let store = self.0.clone();
        tokio::task::spawn_blocking(move || {
            let mut database = store.lock().map_err(|_| Error::Stopped)?;
            f(&mut database).map_err(Error::from)
        })
        .await
        .map_err(|_| Error::Stopped)?
    }
}
