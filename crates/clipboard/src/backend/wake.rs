use crate::Result;
use std::sync::{Condvar, Mutex};
#[cfg(any(target_os = "macos", test))]
use std::time::Duration;

/// A level-triggered wake hint. Request contents stay in the runtime's queue.
pub(crate) struct Wake {
    pending: Mutex<bool>,
    condition: Condvar,
    #[cfg(target_os = "linux")]
    reader: std::os::unix::net::UnixStream,
    #[cfg(target_os = "linux")]
    writer: std::os::unix::net::UnixStream,
    #[cfg(target_os = "windows")]
    event: usize,
}
impl Wake {
    pub fn new() -> Result<Self> {
        #[cfg(target_os = "linux")]
        let (reader, writer) = {
            let (reader, writer) = std::os::unix::net::UnixStream::pair()
                .map_err(|e| crate::Error::backend("create wake socket", e))?;
            reader
                .set_nonblocking(true)
                .map_err(|e| crate::Error::backend("configure wake socket", e))?;
            writer
                .set_nonblocking(true)
                .map_err(|e| crate::Error::backend("configure wake socket", e))?;
            (reader, writer)
        };
        #[cfg(target_os = "windows")]
        let event = {
            // SAFETY: unnamed auto-reset event with no borrowed resources.
            let event = unsafe {
                windows_sys::Win32::System::Threading::CreateEventW(
                    std::ptr::null(),
                    0,
                    0,
                    std::ptr::null(),
                )
            };
            if event.is_null() {
                return Err(crate::Error::backend(
                    "create wake event",
                    std::io::Error::last_os_error(),
                ));
            }
            event as usize
        };
        Ok(Self {
            pending: Mutex::new(false),
            condition: Condvar::new(),
            #[cfg(target_os = "linux")]
            reader,
            #[cfg(target_os = "linux")]
            writer,
            #[cfg(target_os = "windows")]
            event,
        })
    }
    pub fn notify(&self) {
        *self.pending.lock().unwrap_or_else(|e| e.into_inner()) = true;
        self.condition.notify_one();
        #[cfg(target_os = "linux")]
        {
            use std::io::Write;
            let _ = (&self.writer).write(&[1]);
        }
        #[cfg(target_os = "windows")]
        // SAFETY: the event remains alive while this shared wake object is borrowed.
        unsafe {
            windows_sys::Win32::System::Threading::SetEvent(self.event as _);
        }
    }
    #[cfg(any(target_os = "macos", test))]
    pub fn wait(&self, timeout: Duration) {
        let pending = self.pending.lock().unwrap_or_else(|e| e.into_inner());
        let (mut pending, _) = self
            .condition
            .wait_timeout_while(pending, timeout, |p| !*p)
            .unwrap_or_else(|e| e.into_inner());
        *pending = false;
    }
    #[cfg(target_os = "linux")]
    pub fn fd(&self) -> std::os::fd::RawFd {
        use std::os::fd::AsRawFd;
        self.reader.as_raw_fd()
    }
    #[cfg(target_os = "linux")]
    pub fn drain(&self) {
        use std::io::Read;
        let mut bytes = [0; 64];
        while matches!((&self.reader).read(&mut bytes), Ok(1..)) {}
    }
    #[cfg(target_os = "windows")]
    pub fn handle(&self) -> windows_sys::Win32::Foundation::HANDLE {
        self.event as _
    }
}
#[cfg(target_os = "windows")]
impl Drop for Wake {
    fn drop(&mut self) {
        // SAFETY: this is the sole owner, and all shared references have been dropped.
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.event as _);
        }
    }
}
