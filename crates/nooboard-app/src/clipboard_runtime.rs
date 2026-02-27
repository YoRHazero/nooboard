use std::sync::Arc;

use nooboard_platform::ClipboardBackend;

use crate::AppResult;

pub trait ClipboardPort: Send + Sync {
    fn read_text(&self) -> AppResult<Option<String>>;
    fn write_text(&self, text: &str) -> AppResult<()>;
}

impl<T> ClipboardPort for T
where
    T: ClipboardBackend,
{
    fn read_text(&self) -> AppResult<Option<String>> {
        ClipboardBackend::read_text(self).map_err(Into::into)
    }

    fn write_text(&self, text: &str) -> AppResult<()> {
        ClipboardBackend::write_text(self, text).map_err(Into::into)
    }
}

#[derive(Clone)]
pub struct ClipboardRuntime {
    backend: Arc<dyn ClipboardPort>,
}

impl ClipboardRuntime {
    pub fn new(backend: Arc<dyn ClipboardPort>) -> Self {
        Self { backend }
    }

    pub fn read_text(&self) -> AppResult<Option<String>> {
        self.backend.read_text()
    }

    pub fn write_text(&self, text: &str) -> AppResult<()> {
        self.backend.write_text(text)
    }
}
