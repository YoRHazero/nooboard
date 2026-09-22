use super::native_error;
use crate::Result;
use windows_sys::Win32::{
    Foundation::{GlobalFree, HGLOBAL},
    System::{DataExchange::CloseClipboard, Memory::*},
};

pub(super) struct ClipboardGuard;
impl Drop for ClipboardGuard {
    fn drop(&mut self) {
        // SAFETY: only constructed after this thread successfully opens the clipboard.
        unsafe {
            CloseClipboard();
        }
    }
}
pub(super) struct GlobalBytes(pub HGLOBAL);
impl GlobalBytes {
    pub fn new(bytes: &[u8]) -> Result<Self> {
        // SAFETY: allocation matches the buffer, and the guard frees all failure paths.
        unsafe {
            let handle = Self(GlobalAlloc(GMEM_MOVEABLE, bytes.len()));
            if handle.0.is_null() {
                return Err(native_error("allocate clipboard memory"));
            }
            let pointer = GlobalLock(handle.0).cast::<u8>();
            if pointer.is_null() {
                return Err(native_error("lock clipboard memory"));
            }
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), pointer, bytes.len());
            GlobalUnlock(handle.0);
            Ok(handle)
        }
    }
}
impl Drop for GlobalBytes {
    fn drop(&mut self) {
        // SAFETY: non-null handles have not been transferred to Windows.
        unsafe {
            if !self.0.is_null() {
                GlobalFree(self.0);
            }
        }
    }
}
