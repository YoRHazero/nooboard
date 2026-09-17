use crate::{Content, Error, Origin, Result, Snapshot, worker::Command};
use crate::{ImageData, ImageEncoding, MAX_IMAGE_BYTES};
use std::{ptr::null_mut, sync::mpsc, time::Duration};
use windows_sys::{
    Win32::{
        Foundation::{
            CloseHandle, ERROR_CLASS_ALREADY_EXISTS, GetLastError, GlobalFree, HANDLE, HWND,
            WAIT_FAILED,
        },
        System::{DataExchange::*, LibraryLoader::GetModuleHandleW, Memory::*, Threading::*},
        UI::{Shell::DragQueryFileW, WindowsAndMessaging::*},
    },
    core::w,
};

const TEXT: u32 = 13; // CF_UNICODETEXT

/// Owns an auto-reset kernel event; the integer representation can cross threads.
pub(crate) struct Wake(usize);
impl Wake {
    pub fn new() -> Result<Self> {
        // SAFETY: unnamed event, no security descriptor or borrowed resources.
        let handle = unsafe { CreateEventW(std::ptr::null(), 0, 0, std::ptr::null()) };
        if handle.is_null() {
            Err(Error::Native)
        } else {
            Ok(Self(handle as usize))
        }
    }
    pub fn notify(&self) {
        // SAFETY: this event remains owned by self until all Arc references are dropped.
        unsafe {
            SetEvent(self.0 as HANDLE);
        }
    }
}
impl Drop for Wake {
    fn drop(&mut self) {
        // SAFETY: exactly one owner releases the event after the worker exits.
        unsafe {
            CloseHandle(self.0 as HANDLE);
        }
    }
}

struct ClipboardGuard;
impl Drop for ClipboardGuard {
    fn drop(&mut self) {
        // SAFETY: constructed only after OpenClipboard succeeds on this thread.
        unsafe {
            CloseClipboard();
        }
    }
}

pub(crate) struct Native {
    window: HWND,
    max_bytes: usize,
}
impl Native {
    pub fn open(max_bytes: usize) -> Result<Self> {
        // SAFETY: all pointers refer to static strings or initialized stack structures;
        // the message-only window and its listener live entirely on this worker thread.
        unsafe {
            let instance = GetModuleHandleW(std::ptr::null());
            let class = WNDCLASSW {
                lpfnWndProc: Some(DefWindowProcW),
                hInstance: instance,
                lpszClassName: w!("NooboardClipboardV1"),
                ..std::mem::zeroed()
            };
            if RegisterClassW(&class) == 0 && GetLastError() != ERROR_CLASS_ALREADY_EXISTS {
                return Err(Error::Native);
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
                return Err(Error::Native);
            }
            if AddClipboardFormatListener(window) == 0 {
                DestroyWindow(window);
                return Err(Error::Native);
            }
            Ok(Self { window, max_bytes })
        }
    }
    pub fn revision(&self) -> u64 {
        // SAFETY: the sequence number has no pointer or ownership preconditions.
        unsafe { GetClipboardSequenceNumber() as u64 }
    }
    pub fn wait(
        &mut self,
        rx: &mpsc::Receiver<Command>,
        wake: &Wake,
        _: Duration,
        retry: bool,
    ) -> Result<Option<Command>> {
        match rx.try_recv() {
            Ok(command) => return Ok(Some(command)),
            Err(mpsc::TryRecvError::Disconnected) => return Err(Error::Stopped),
            Err(_) => {}
        }
        // SAFETY: event is alive, message queue belongs to this worker, MSG is writable.
        unsafe {
            let handle = wake.0 as HANDLE;
            let timeout = if retry { 10 } else { INFINITE };
            if MsgWaitForMultipleObjectsEx(1, &handle, timeout, QS_ALLINPUT, MWMO_INPUTAVAILABLE)
                == WAIT_FAILED
            {
                return Err(Error::Native);
            }
            let mut message = std::mem::zeroed();
            while PeekMessageW(&mut message, null_mut(), 0, 0, PM_REMOVE) != 0 {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        match rx.try_recv() {
            Ok(command) => Ok(Some(command)),
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(_) => Err(Error::Stopped),
        }
    }
    fn lock(&self) -> Result<ClipboardGuard> {
        for _ in 0..10 {
            // SAFETY: window is a valid message-only window owned by this thread.
            if unsafe { OpenClipboard(self.window) } != 0 {
                return Ok(ClipboardGuard);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        Err(Error::Unavailable)
    }
    pub fn read(&self) -> Result<Snapshot> {
        let _guard = self.lock()?;
        let revision = self.revision();
        // SAFETY: clipboard is open for this thread, static names are NUL-terminated;
        // borrowed HGLOBAL data is bounded by GlobalSize and unlocked before returning.
        let content = unsafe {
            let registered = |name| RegisterClipboardFormatW(name);
            let present = |format| format != 0 && IsClipboardFormatAvailable(format) != 0;
            if present(registered(w!(
                "ExcludeClipboardContentFromMonitorProcessing"
            ))) {
                Content::Sensitive
            } else if present(15) {
                use std::os::windows::ffi::OsStringExt;
                let handle = GetClipboardData(15);
                if handle.is_null() {
                    return Err(Error::Unavailable);
                }
                let count = DragQueryFileW(handle, u32::MAX, std::ptr::null_mut(), 0);
                if count == 0 || count > 256 {
                    return Err(Error::InvalidInput);
                }
                let mut files = Vec::new();
                for index in 0..count {
                    let len = DragQueryFileW(handle, index, std::ptr::null_mut(), 0);
                    if len == 0 || len > 32767 {
                        return Err(Error::InvalidInput);
                    }
                    let mut path = vec![0u16; len as usize + 1];
                    if DragQueryFileW(handle, index, path.as_mut_ptr(), len + 1) != len {
                        return Err(Error::Native);
                    }
                    files.push(std::path::PathBuf::from(std::ffi::OsString::from_wide(
                        &path[..len as usize],
                    )));
                }
                Content::Files(files)
            } else if [
                registered(w!("FileGroupDescriptorW")),
                registered(w!("FileGroupDescriptor")),
                registered(w!("FileContents")),
            ]
            .into_iter()
            .any(present)
            {
                Content::Unsupported
            } else if present(registered(w!("PNG"))) {
                match read_bytes(registered(w!("PNG")), MAX_IMAGE_BYTES)? {
                    Some(bytes) => Content::Image(ImageData::new(ImageEncoding::Png, bytes)?),
                    None => Content::TooLarge,
                }
            } else if present(17) || present(8) {
                match read_bytes(if present(17) { 17 } else { 8 }, 256 * 1024 * 1024 + 124)? {
                    Some(bytes) => Content::Image(ImageData::from_dib(&bytes)?),
                    None => Content::TooLarge,
                }
            } else if present(registered(w!("JFIF"))) {
                match read_bytes(registered(w!("JFIF")), MAX_IMAGE_BYTES)? {
                    Some(bytes) => Content::Image(ImageData::new(ImageEncoding::Jpeg, bytes)?),
                    None => Content::TooLarge,
                }
            } else if present(2) {
                Content::Unsupported
            } else if present(TEXT) {
                let handle = GetClipboardData(TEXT);
                if handle.is_null() {
                    return Err(Error::Unavailable);
                }
                let size = GlobalSize(handle);
                if size > self.max_bytes.saturating_mul(2).saturating_add(2) {
                    Content::TooLarge
                } else if size < 2 || !size.is_multiple_of(2) {
                    return Err(Error::Native);
                } else {
                    let ptr = GlobalLock(handle) as *const u16;
                    if ptr.is_null() {
                        return Err(Error::Native);
                    }
                    let data = std::slice::from_raw_parts(ptr, size / 2);
                    let result = match data.iter().position(|&c| c == 0) {
                        Some(end) => String::from_utf16(&data[..end]).map_err(|_| Error::Native),
                        None => Err(Error::Native),
                    };
                    GlobalUnlock(handle);
                    let text = result?;
                    if text.len() > self.max_bytes {
                        Content::TooLarge
                    } else {
                        Content::Text(text)
                    }
                }
            } else if CountClipboardFormats() == 0 {
                Content::Empty
            } else {
                Content::Unsupported
            }
        };
        if self.revision() != revision {
            return Err(Error::Changed);
        }
        Ok(Snapshot {
            revision,
            content,
            origin: Origin::External,
        })
    }
    pub fn write(&mut self, text: &str) -> Result<Snapshot> {
        if text.len() > self.max_bytes || text.contains('\0') {
            return Err(Error::InvalidInput);
        }
        let encoded: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
        let _guard = self.lock()?;
        // SAFETY: allocated size matches the UTF-16 buffer. Ownership transfers to the
        // system only after SetClipboardData succeeds; all failure paths free it.
        unsafe {
            let handle = GlobalAlloc(GMEM_MOVEABLE, encoded.len() * 2);
            if handle.is_null() {
                return Err(Error::Native);
            }
            let ptr = GlobalLock(handle) as *mut u16;
            if ptr.is_null() {
                GlobalFree(handle);
                return Err(Error::Native);
            }
            std::ptr::copy_nonoverlapping(encoded.as_ptr(), ptr, encoded.len());
            GlobalUnlock(handle);
            if EmptyClipboard() == 0 || SetClipboardData(TEXT, handle).is_null() {
                GlobalFree(handle);
                return Err(Error::Native);
            }
        }
        Ok(Snapshot {
            revision: self.revision(),
            content: Content::Text(text.into()),
            origin: Origin::Application,
        })
    }
    pub fn write_content(&mut self, content: &Content) -> Result<Snapshot> {
        if let Content::Text(text) = content {
            return self.write(text);
        }
        let mut formats = Vec::new();
        match content {
            Content::Image(image) if image.encoding == ImageEncoding::Png => {
                let dib = image.dib_v5()?;
                // SAFETY: static terminated registered format name.
                let png = unsafe { RegisterClipboardFormatW(w!("PNG")) };
                if png == 0 {
                    return Err(Error::Native);
                }
                formats.push((png, image.bytes.as_ref().clone()));
                formats.push((17, dib));
            }
            Content::Files(files) => {
                use std::os::windows::ffi::OsStrExt;
                if files.is_empty() || files.len() > 256 {
                    return Err(Error::InvalidInput);
                }
                let mut data = vec![0u8; 20];
                data[0..4].copy_from_slice(&20u32.to_le_bytes());
                data[16..20].copy_from_slice(&1u32.to_le_bytes());
                for path in files {
                    if !path.is_absolute() {
                        return Err(Error::InvalidInput);
                    }
                    for c in path.as_os_str().encode_wide() {
                        if c == 0 {
                            return Err(Error::InvalidInput);
                        }
                        data.extend_from_slice(&c.to_le_bytes());
                    }
                    data.extend_from_slice(&[0, 0]);
                }
                data.extend_from_slice(&[0, 0]);
                formats.push((15, data));
                // SAFETY: static terminated registered format name; COPY never deletes source files.
                let effect = unsafe { RegisterClipboardFormatW(w!("Preferred DropEffect")) };
                if effect == 0 {
                    return Err(Error::Native);
                }
                formats.push((effect, 1u32.to_le_bytes().to_vec()));
            }
            _ => return Err(Error::InvalidInput),
        }
        let mut handles = Vec::new();
        for (format, bytes) in formats {
            handles.push((format, GlobalBytes::new(&bytes)?));
        }
        let _guard = self.lock()?;
        // SAFETY: this thread owns the open clipboard and every HGLOBAL until successful transfer.
        unsafe {
            if EmptyClipboard() == 0 {
                return Err(Error::Native);
            }
            for (format, mut handle) in handles {
                if SetClipboardData(format, handle.0).is_null() {
                    return Err(Error::Native);
                }
                handle.0 = std::ptr::null_mut();
            }
        }
        Ok(Snapshot {
            revision: self.revision(),
            content: content.clone(),
            origin: Origin::Application,
        })
    }
}

fn read_bytes(format: u32, limit: usize) -> Result<Option<Vec<u8>>> {
    // SAFETY: caller holds ClipboardGuard; data is copied within GlobalSize while locked.
    unsafe {
        let handle = GetClipboardData(format);
        if handle.is_null() {
            return Err(Error::Unavailable);
        }
        let len = GlobalSize(handle);
        if len > limit {
            return Ok(None);
        }
        if len == 0 {
            return Err(Error::InvalidInput);
        }
        let ptr = GlobalLock(handle).cast::<u8>();
        if ptr.is_null() {
            return Err(Error::Native);
        }
        let bytes = std::slice::from_raw_parts(ptr, len).to_vec();
        GlobalUnlock(handle);
        Ok(Some(bytes))
    }
}
struct GlobalBytes(windows_sys::Win32::Foundation::HGLOBAL);
impl GlobalBytes {
    fn new(bytes: &[u8]) -> Result<Self> {
        // SAFETY: buffer is allocated to the exact copy size, unlocked before use by the clipboard.
        unsafe {
            let handle = Self(GlobalAlloc(GMEM_MOVEABLE, bytes.len()));
            if handle.0.is_null() {
                return Err(Error::Native);
            }
            let ptr = GlobalLock(handle.0).cast::<u8>();
            if ptr.is_null() {
                return Err(Error::Native);
            }
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, bytes.len());
            GlobalUnlock(handle.0);
            Ok(handle)
        }
    }
}
impl Drop for GlobalBytes {
    fn drop(&mut self) {
        // SAFETY: a non-null handle has not transferred to the system and remains uniquely owned.
        unsafe {
            if !self.0.is_null() {
                GlobalFree(self.0);
            }
        }
    }
}
impl Drop for Native {
    fn drop(&mut self) {
        // SAFETY: listener and window are destroyed on the same thread that created them.
        unsafe {
            RemoveClipboardFormatListener(self.window);
            DestroyWindow(self.window);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "writes the clipboard; run only in a disposable Windows test session"]
    fn native_text_roundtrip_on_disposable_desktop() {
        let mut native = Native::open(1024).unwrap();
        let original = native.read().unwrap();
        let old_text = match original.content {
            Content::Text(text) => text,
            Content::Empty => String::new(),
            _ => panic!("use a disposable desktop with an empty or plain-text clipboard"),
        };
        let text = " 中文 🦀\r\n code\n ";
        let write = native.write(text).unwrap();
        let read = native.read().unwrap();
        native.write(&old_text).unwrap();
        assert_eq!(write.origin, Origin::Application);
        assert_eq!(read.content, Content::Text(text.into()));
        assert_ne!(read.revision, original.revision);
    }
}
