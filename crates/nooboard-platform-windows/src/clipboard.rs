use std::mem;
use std::ptr::{copy_nonoverlapping, null, null_mut};
use std::slice;
use std::sync::LazyLock;
use std::thread;
use std::time::Duration;

use nooboard_platform::NooboardError;
use windows_sys::Win32::Foundation::{GetLastError, GlobalFree, HGLOBAL, HWND};
use windows_sys::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, GetClipboardSequenceNumber,
    IsClipboardFormatAvailable, OpenClipboard, SetClipboardData,
};
use windows_sys::Win32::System::Memory::{
    GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock,
};
use windows_sys::Win32::System::Ole::CF_UNICODETEXT;
use windows_sys::Win32::UI::WindowsAndMessaging::{CreateWindowExW, DestroyWindow, HWND_MESSAGE};

use crate::encoding::{decode_cf_unicode_text, encode_cf_unicode_text};

const OPEN_CLIPBOARD_RETRIES: usize = 8;
const OPEN_CLIPBOARD_RETRY_DELAY: Duration = Duration::from_millis(10);

static OWNER_WINDOW_TITLE: LazyLock<Vec<u16>> = LazyLock::new(|| {
    "Nooboard Clipboard Owner"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect()
});

static STATIC_WINDOW_CLASS: LazyLock<Vec<u16>> =
    LazyLock::new(|| "STATIC".encode_utf16().chain(std::iter::once(0)).collect());

pub(crate) fn read_text_from_clipboard() -> Result<Option<String>, NooboardError> {
    let _clipboard = ClipboardGuard::open_for_read()?;

    if unsafe { IsClipboardFormatAvailable(unicode_text_format()) } == 0 {
        return Ok(None);
    }

    let handle = unsafe { GetClipboardData(unicode_text_format()) };
    if handle.is_null() {
        return Err(last_win32_error("GetClipboardData(CF_UNICODETEXT)"));
    }

    let size_bytes = unsafe { GlobalSize(handle.cast()) };
    if size_bytes == 0 {
        return Err(NooboardError::platform(
            "clipboard text buffer has zero size",
        ));
    }
    if size_bytes % mem::size_of::<u16>() != 0 {
        return Err(NooboardError::platform(
            "clipboard text buffer is not UTF-16 aligned",
        ));
    }

    let ptr = unsafe { GlobalLock(handle.cast()) }.cast::<u16>();
    if ptr.is_null() {
        return Err(last_win32_error("GlobalLock(clipboard text)"));
    }

    let units_len = size_bytes / mem::size_of::<u16>();
    let text = unsafe {
        let units = slice::from_raw_parts(ptr.cast_const(), units_len);
        decode_cf_unicode_text(units)
    };

    let _ = unsafe { GlobalUnlock(handle.cast()) };
    text.map(Some)
}

pub(crate) fn write_text_to_clipboard(text: &str) -> Result<(), NooboardError> {
    let encoded = encode_cf_unicode_text(text)?;
    let size_bytes = encoded
        .len()
        .checked_mul(mem::size_of::<u16>())
        .ok_or_else(|| NooboardError::platform("clipboard text is too large to encode"))?;

    let memory = OwnedGlobalMemory::allocate(size_bytes)?;
    let ptr = unsafe { GlobalLock(memory.handle()) }.cast::<u16>();
    if ptr.is_null() {
        return Err(last_win32_error("GlobalLock(clipboard write buffer)"));
    }

    unsafe {
        copy_nonoverlapping(encoded.as_ptr(), ptr, encoded.len());
        let _ = GlobalUnlock(memory.handle());
    }

    let _clipboard = ClipboardGuard::open_for_write()?;
    if unsafe { EmptyClipboard() } == 0 {
        return Err(last_win32_error("EmptyClipboard"));
    }

    let handle = unsafe { SetClipboardData(unicode_text_format(), memory.into_handle().cast()) };
    if handle.is_null() {
        return Err(last_win32_error("SetClipboardData(CF_UNICODETEXT)"));
    }

    Ok(())
}

pub(crate) fn clipboard_sequence_number() -> u32 {
    unsafe { GetClipboardSequenceNumber() }
}

struct ClipboardGuard {
    _owner_window: Option<ClipboardOwnerWindow>,
}

impl ClipboardGuard {
    fn open_for_read() -> Result<Self, NooboardError> {
        Self::open(None)
    }

    fn open_for_write() -> Result<Self, NooboardError> {
        Self::open(Some(ClipboardOwnerWindow::new()?))
    }

    fn open(owner_window: Option<ClipboardOwnerWindow>) -> Result<Self, NooboardError> {
        let hwnd = owner_window
            .as_ref()
            .map(ClipboardOwnerWindow::handle)
            .unwrap_or(null_mut());
        let mut last_error = None;

        for attempt in 0..OPEN_CLIPBOARD_RETRIES {
            if unsafe { OpenClipboard(hwnd) } != 0 {
                return Ok(Self {
                    _owner_window: owner_window,
                });
            }

            last_error = Some(last_win32_error("OpenClipboard"));
            if attempt + 1 < OPEN_CLIPBOARD_RETRIES {
                thread::sleep(OPEN_CLIPBOARD_RETRY_DELAY);
            }
        }

        Err(last_error.unwrap_or_else(|| NooboardError::platform("OpenClipboard failed")))
    }
}

impl Drop for ClipboardGuard {
    fn drop(&mut self) {
        let _ = unsafe { CloseClipboard() };
    }
}

struct ClipboardOwnerWindow {
    handle: HWND,
}

impl ClipboardOwnerWindow {
    fn new() -> Result<Self, NooboardError> {
        let handle = unsafe {
            CreateWindowExW(
                0,
                STATIC_WINDOW_CLASS.as_ptr(),
                OWNER_WINDOW_TITLE.as_ptr(),
                0,
                0,
                0,
                0,
                0,
                HWND_MESSAGE,
                null_mut(),
                null_mut(),
                null(),
            )
        };

        if handle.is_null() {
            return Err(last_win32_error("CreateWindowExW(clipboard owner window)"));
        }

        Ok(Self { handle })
    }

    fn handle(&self) -> HWND {
        self.handle
    }
}

impl Drop for ClipboardOwnerWindow {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            let _ = unsafe { DestroyWindow(self.handle) };
        }
    }
}

struct OwnedGlobalMemory {
    handle: HGLOBAL,
}

impl OwnedGlobalMemory {
    fn allocate(size_bytes: usize) -> Result<Self, NooboardError> {
        let handle = unsafe { GlobalAlloc(GMEM_MOVEABLE, size_bytes) };
        if handle.is_null() {
            return Err(last_win32_error("GlobalAlloc"));
        }
        Ok(Self { handle })
    }

    fn handle(&self) -> HGLOBAL {
        self.handle
    }

    fn into_handle(mut self) -> HGLOBAL {
        let handle = self.handle;
        self.handle = null_mut();
        handle
    }
}

impl Drop for OwnedGlobalMemory {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            let _ = unsafe { GlobalFree(self.handle) };
        }
    }
}

fn last_win32_error(operation: &str) -> NooboardError {
    let code = unsafe { GetLastError() };
    if code == 0 {
        return NooboardError::platform(format!("{operation} failed"));
    }

    let detail = std::io::Error::from_raw_os_error(code as i32);
    NooboardError::platform(format!(
        "{operation} failed with Win32 error {code}: {detail}"
    ))
}

fn unicode_text_format() -> u32 {
    u32::from(CF_UNICODETEXT)
}
