//! Private native driver contract. This module does not know about API requests,
//! response channels, subscriptions, or the public observation sequence.
mod wake;
use crate::{Limits, Options, Origin, Payload, ReadState, Result, formats};
use std::time::Duration;
pub(crate) use wake::Wake;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

/// Owned bytes prepared off the native thread. Contains no native handles.
pub(crate) struct PreparedWrite {
    pub payload: Payload,
    #[cfg(target_os = "windows")]
    pub dib: Option<Vec<u8>>,
}
pub(crate) fn prepare(payload: Payload, limits: &Limits) -> Result<PreparedWrite> {
    let payload = formats::normalize(payload, limits)?;
    #[cfg(target_os = "windows")]
    let dib = match &payload {
        Payload::Image(image) => Some(windows::formats::dib_v5(image)?),
        _ => None,
    };
    Ok(PreparedWrite {
        payload,
        #[cfg(target_os = "windows")]
        dib,
    })
}

pub(crate) struct Observation {
    pub revision: u64,
    pub content: ReadState,
    pub origin: Origin,
}
pub(crate) enum Operation {
    Read,
    Write(PreparedWrite),
}
pub(crate) enum Progress {
    Idle,
    #[cfg(any(target_os = "linux", test))]
    Pending,
    Complete(Result<Observation>),
}

/// Implementations are created, polled, and dropped on their owning thread.
/// `poll` performs a bounded pass, including serving data while no operation is active.
pub(crate) trait Backend {
    fn revision(&self) -> u64;
    fn begin(&mut self, operation: Operation) -> Result<()>;
    fn poll(&mut self) -> Result<Progress>;
    fn cancel(&mut self);
    fn wait(&mut self, wake: &Wake, timeout: Duration) -> Result<()>;
}

pub(crate) fn open(options: &Options) -> Result<Box<dyn Backend>> {
    #[cfg(target_os = "macos")]
    return Ok(Box::new(ImmediateBackend::new(macos::Native::open(
        options,
    )?)));
    #[cfg(target_os = "windows")]
    return Ok(Box::new(ImmediateBackend::new(windows::Native::open(
        options,
    )?)));
    #[cfg(target_os = "linux")]
    return linux::open(options);
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    Err(crate::Error::UnsupportedPlatform)
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub(super) trait ImmediateNative {
    fn pump(&mut self) {}
    fn revision(&self) -> u64;
    fn read(&mut self) -> Result<Observation>;
    fn write(&mut self, prepared: PreparedWrite) -> Result<Observation>;
    fn wait(&mut self, wake: &Wake, timeout: Duration) -> Result<()>;
}
#[cfg(any(target_os = "macos", target_os = "windows"))]
struct ImmediateBackend<N> {
    native: N,
    operation: Option<Operation>,
}
#[cfg(any(target_os = "macos", target_os = "windows"))]
impl<N> ImmediateBackend<N> {
    fn new(native: N) -> Self {
        Self {
            native,
            operation: None,
        }
    }
}
#[cfg(any(target_os = "macos", target_os = "windows"))]
impl<N: ImmediateNative> Backend for ImmediateBackend<N> {
    fn revision(&self) -> u64 {
        self.native.revision()
    }
    fn begin(&mut self, operation: Operation) -> Result<()> {
        debug_assert!(self.operation.is_none());
        self.operation = Some(operation);
        Ok(())
    }
    fn poll(&mut self) -> Result<Progress> {
        self.native.pump();
        Ok(match self.operation.take() {
            None => Progress::Idle,
            Some(Operation::Read) => Progress::Complete(self.native.read()),
            Some(Operation::Write(payload)) => Progress::Complete(self.native.write(payload)),
        })
    }
    fn cancel(&mut self) {
        self.operation = None;
    }
    fn wait(&mut self, wake: &Wake, timeout: Duration) -> Result<()> {
        self.native.wait(wake, timeout)
    }
}
