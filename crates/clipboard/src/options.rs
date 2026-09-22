use crate::{Error, MAX_IMAGE_BYTES, Result};
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct Limits {
    pub text_bytes: usize,
    /// Maximum image bytes accepted by the service, including native DIB data
    /// wrapped as BMP. Native read limits must also be satisfied.
    pub image_bytes: usize,
    pub files: usize,
    pub file_list_bytes: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            text_bytes: 1024 * 1024,
            image_bytes: MAX_IMAGE_BYTES,
            files: 256,
            file_list_bytes: 4 * 1024 * 1024,
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BackendPreference {
    #[default]
    Automatic,
    X11,
    Wayland,
}
#[derive(Clone, Debug)]
pub struct Options {
    pub limits: Limits,
    pub backend: BackendPreference,
    /// Used by backends without native change notifications (currently macOS).
    pub poll_interval: Duration,
    /// Overall deadline for one native read or write, including read retries.
    pub operation_timeout: Duration,
    pub retry_interval: Duration,
    pub queue_capacity: usize,
    /// Only macOS supports a named diagnostic pasteboard.
    #[cfg(feature = "diagnostics")]
    pub diagnostic_name: Option<String>,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            limits: Limits::default(),
            backend: BackendPreference::Automatic,
            poll_interval: Duration::from_millis(250),
            operation_timeout: Duration::from_secs(5),
            retry_interval: Duration::from_millis(25),
            queue_capacity: 16,
            #[cfg(feature = "diagnostics")]
            diagnostic_name: None,
        }
    }
}
impl Options {
    pub(crate) fn validate(&self) -> Result<()> {
        if self.limits.text_bytes == 0
            || self.limits.image_bytes == 0
            || self.limits.image_bytes > MAX_IMAGE_BYTES
            || self.limits.files == 0
            || self.limits.files > 256
            || self.limits.file_list_bytes == 0
            || self.poll_interval.is_zero()
            || self.operation_timeout.is_zero()
            || self.retry_interval.is_zero()
            || self.queue_capacity == 0
            || self.queue_capacity > 1024
            || self.operation_timeout > Duration::from_secs(300)
            || self.poll_interval > Duration::from_secs(60)
            || self.retry_interval > self.operation_timeout
        {
            return Err(Error::InvalidInput);
        }
        #[cfg(not(target_os = "linux"))]
        if self.backend != BackendPreference::Automatic {
            return Err(Error::UnsupportedPlatform);
        }
        #[cfg(feature = "diagnostics")]
        if let Some(name) = &self.diagnostic_name
            && (!name.starts_with("nooboard.diagnostic.") || name.contains('\0'))
        {
            return Err(Error::InvalidInput);
        }
        #[cfg(all(feature = "diagnostics", not(target_os = "macos")))]
        if self.diagnostic_name.is_some() {
            return Err(Error::UnsupportedPlatform);
        }
        Ok(())
    }
}
