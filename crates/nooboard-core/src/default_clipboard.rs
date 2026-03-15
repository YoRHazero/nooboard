use std::sync::Arc;

use crate::clipboard::port::ClipboardPort;
use crate::error::CoreResult;

pub(crate) fn default_clipboard_port() -> CoreResult<Arc<dyn ClipboardPort>> {
    #[cfg(target_os = "macos")]
    {
        return Ok(Arc::new(
            nooboard_platform_macos::MacOsClipboardBackend::new(),
        ));
    }

    #[cfg(target_os = "windows")]
    {
        return Ok(Arc::new(
            nooboard_platform_windows::WindowsClipboardBackend::new(),
        ));
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Err(CoreError::Clipboard(
            nooboard_platform::NooboardError::UnsupportedPlatform.to_string(),
        ))
    }
}
