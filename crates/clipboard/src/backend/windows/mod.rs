use crate::backend::PreparedWrite;
pub(crate) mod formats;
mod handles;
use super::{ImmediateNative, Observation, Wake};
use crate::{Error, Options, Origin, ReadState, Result};
use handles::{ClipboardGuard, GlobalBytes};
use std::{ptr::null_mut, time::Duration};
use windows_sys::{
    Win32::{
        Foundation::{ERROR_CLASS_ALREADY_EXISTS, GetLastError, HWND, WAIT_FAILED},
        System::{DataExchange::*, LibraryLoader::GetModuleHandleW},
        UI::WindowsAndMessaging::*,
    },
    core::w,
};

fn native_error(operation: &'static str) -> Error {
    Error::backend(operation, std::io::Error::last_os_error())
}
pub(super) struct Native {
    window: HWND,
    limits: crate::Limits,
}
impl Native {
    pub fn open(options: &Options) -> Result<Self> {
        // SAFETY: all pointers refer to static strings or initialized structures.
        // The message-only window and listener stay on this native worker thread.
        unsafe {
            let instance = GetModuleHandleW(std::ptr::null());
            let class = WNDCLASSW {
                lpfnWndProc: Some(DefWindowProcW),
                hInstance: instance,
                lpszClassName: w!("NooboardClipboardV1"),
                ..std::mem::zeroed()
            };
            if RegisterClassW(&class) == 0 && GetLastError() != ERROR_CLASS_ALREADY_EXISTS {
                return Err(native_error("register clipboard window"));
            }
            let window = CreateWindowExW(
                0,
                class.lpszClassName,
                w!(""),
                0,
                0,
                0,
                0,
                0,
                HWND_MESSAGE,
                null_mut(),
                instance,
                std::ptr::null(),
            );
            if window.is_null() {
                return Err(native_error("create clipboard window"));
            }
            if AddClipboardFormatListener(window) == 0 {
                let error = native_error("listen for clipboard changes");
                DestroyWindow(window);
                return Err(error);
            }
            Ok(Self {
                window,
                limits: options.limits.clone(),
            })
        }
    }
    fn lock(&self) -> Result<ClipboardGuard> {
        // A bounded native critical section. Large data transfers use owned buffers.
        for _ in 0..10 {
            // SAFETY: window is alive and belongs to this worker thread.
            if unsafe { OpenClipboard(self.window) } != 0 {
                return Ok(ClipboardGuard);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        Err(Error::Unavailable)
    }
}
impl ImmediateNative for Native {
    fn revision(&self) -> u64 {
        // SAFETY: sequence lookup has no pointer or ownership preconditions.
        unsafe { GetClipboardSequenceNumber() as u64 }
    }
    fn pump(&mut self) {
        // SAFETY: this thread owns the queue; the stack MSG is writable.
        unsafe {
            let mut message = std::mem::zeroed();
            for _ in 0..256 {
                if PeekMessageW(&mut message, null_mut(), 0, 0, PM_REMOVE) == 0 {
                    break;
                }
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }
    fn wait(&mut self, wake: &Wake, timeout: Duration) -> Result<()> {
        let handle = wake.handle();
        // SAFETY: wake event is alive, and this thread owns the native message queue.
        if unsafe {
            MsgWaitForMultipleObjectsEx(
                1,
                &handle,
                timeout.as_millis().min((u32::MAX - 1) as u128) as u32,
                QS_ALLINPUT,
                MWMO_INPUTAVAILABLE,
            )
        } == WAIT_FAILED
        {
            return Err(native_error("wait for clipboard events"));
        }
        Ok(())
    }
    fn read(&mut self) -> Result<Observation> {
        let guard = self.lock()?;
        let revision = self.revision();
        let content = formats::read(&guard, &self.limits)?;
        if revision != self.revision() {
            return Err(Error::Changed);
        }
        Ok(Observation {
            revision,
            content,
            origin: Origin::External,
        })
    }
    fn write(&mut self, mut prepared: PreparedWrite) -> Result<Observation> {
        let formats = formats::encode(&mut prepared)?;
        let mut handles = Vec::with_capacity(formats.len());
        for (format, bytes) in formats {
            handles.push((format, GlobalBytes::new(&bytes)?));
        }
        let _guard = self.lock()?;
        // SAFETY: the clipboard is open on this thread. Each HGLOBAL remains owned
        // by its guard until successful transfer to the operating system.
        unsafe {
            if EmptyClipboard() == 0 {
                return Err(native_error("empty clipboard"));
            }
            for (format, mut handle) in handles {
                if SetClipboardData(format, handle.0).is_null() {
                    return Err(native_error("publish clipboard format"));
                }
                handle.0 = null_mut();
            }
        }
        Ok(Observation {
            revision: self.revision(),
            content: ReadState::Ready(prepared.payload),
            origin: Origin::Application,
        })
    }
}
impl Drop for Native {
    fn drop(&mut self) {
        // SAFETY: both resources belong to this worker, which is their only owner.
        unsafe {
            RemoveClipboardFormatListener(self.window);
            DestroyWindow(self.window);
        }
    }
}
