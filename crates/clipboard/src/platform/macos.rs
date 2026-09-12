use crate::{Content, Error, Origin, Result, Snapshot, worker::Command};
use objc2::{
    ClassType, msg_send,
    rc::{Retained, autoreleasepool},
};
use objc2_app_kit::NSPasteboard;
use objc2_foundation::NSString;
use std::{
    sync::{Mutex, MutexGuard, mpsc},
    time::Duration,
};

// AppKit can return the same NSPasteboard object to separate callers. Its type
// cache must not be mutated concurrently by two clipboard workers in this process.
static PASTEBOARD_ACCESS: Mutex<()> = Mutex::new(());
fn access() -> MutexGuard<'static, ()> {
    PASTEBOARD_ACCESS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub(crate) struct Wake;
impl Wake {
    pub fn new() -> Result<Self> {
        Ok(Self)
    }
    pub fn notify(&self) {}
}
pub(crate) struct Native {
    board: Retained<NSPasteboard>,
    max_bytes: usize,
}
impl Native {
    #[cfg(feature = "diagnostics")]
    pub fn open_named(name: &str, max_bytes: usize) -> Result<Self> {
        let _access = access();
        if !name.starts_with("nooboard.diagnostic.") {
            return Err(Error::InvalidInput);
        }
        // SAFETY: documented nullable class method; the prefix reserves test boards.
        let board: Option<Retained<NSPasteboard>> = unsafe {
            msg_send![NSPasteboard::class(), pasteboardWithName: &*NSString::from_str(name)]
        };
        Ok(Self {
            board: board.ok_or(Error::Unavailable)?,
            max_bytes,
        })
    }
    pub fn open(max_bytes: usize) -> Result<Self> {
        let _access = access();
        // SAFETY: documented class method; nullable return handles an unavailable
        // pasteboard service (for example a noninteractive or sandboxed session).
        let board: Option<Retained<NSPasteboard>> =
            unsafe { msg_send![NSPasteboard::class(), generalPasteboard] };
        Ok(Self {
            board: board.ok_or(Error::Unavailable)?,
            max_bytes,
        })
    }
    pub fn revision(&self) -> u64 {
        let _access = access();
        self.revision_inner()
    }
    fn revision_inner(&self) -> u64 {
        self.board.changeCount() as u64
    }
    pub fn wait(
        &mut self,
        rx: &mpsc::Receiver<Command>,
        _: &Wake,
        interval: Duration,
        retry: bool,
    ) -> Result<Option<Command>> {
        let delay = if retry {
            Duration::from_millis(10)
        } else {
            interval
        };
        match rx.recv_timeout(delay) {
            Ok(command) => Ok(Some(command)),
            Err(mpsc::RecvTimeoutError::Timeout) => Ok(None),
            Err(_) => Err(Error::Stopped),
        }
    }
    pub fn read(&self) -> Result<Snapshot> {
        let _access = access();
        self.read_inner()
    }
    fn read_inner(&self) -> Result<Snapshot> {
        autoreleasepool(|_| {
            let revision = self.revision_inner();
            let types: Vec<String> = self
                .board
                .types()
                .map(|v| v.iter().map(|s| s.to_string()).collect())
                .unwrap_or_default();
            let has = |names: &[&str]| types.iter().any(|t| names.contains(&t.as_str()));
            let content = if has(&[
                "org.nspasteboard.ConcealedType",
                "org.nspasteboard.TransientType",
            ]) {
                Content::Sensitive
            } else if has(&[
                "public.file-url",
                "NSFilenamesPboardType",
                "public.png",
                "public.tiff",
                "public.jpeg",
                "public.heic",
                "com.compuserve.gif",
                "com.microsoft.bmp",
            ]) {
                Content::Unsupported
            } else if let Some(text) = self
                .board
                .stringForType(&NSString::from_str("public.utf8-plain-text"))
            {
                if text.len() > self.max_bytes {
                    Content::TooLarge
                } else {
                    let text = text.to_string();
                    if text.len() > self.max_bytes {
                        Content::TooLarge
                    } else {
                        Content::Text(text)
                    }
                }
            } else if types.is_empty() {
                Content::Empty
            } else {
                Content::Unsupported
            };
            if self.revision_inner() != revision {
                return Err(Error::Changed);
            }
            Ok(Snapshot {
                revision,
                content,
                origin: Origin::External,
            })
        })
    }
    pub fn write(&mut self, text: &str) -> Result<Snapshot> {
        let _access = access();
        if text.len() > self.max_bytes || text.contains('\0') {
            return Err(Error::InvalidInput);
        }
        autoreleasepool(|_| {
            self.board.clearContents();
            if !self.board.setString_forType(
                &NSString::from_str(text),
                &NSString::from_str("public.utf8-plain-text"),
            ) {
                return Err(Error::Native);
            }
            let mut snapshot = self.read_inner()?;
            if snapshot.content != Content::Text(text.into()) {
                return Err(Error::Changed);
            }
            snapshot.origin = Origin::Application;
            Ok(snapshot)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires the macOS pasteboard service; uses an isolated pasteboard"]
    fn private_pasteboard_roundtrip_and_image_filter() {
        // SAFETY: documented class method, with a nullable result for unavailable service.
        let board: Option<Retained<NSPasteboard>> =
            unsafe { msg_send![NSPasteboard::class(), pasteboardWithUniqueName] };
        let mut native = Native {
            board: board.expect("pasteboard service unavailable in this session"),
            max_bytes: 1024,
        };
        let text = " 中文 🦀\r\nline\n ";
        let written = native.write(text).unwrap();
        assert_eq!(native.read().unwrap().content, Content::Text(text.into()));
        assert_eq!(written.origin, Origin::Application);
        native.board.setString_forType(
            &NSString::from_str("marker"),
            &NSString::from_str("public.file-url"),
        );
        assert_eq!(native.read().unwrap().content, Content::Unsupported);
        native.board.clearContents();
    }
    #[cfg(feature = "diagnostics")]
    #[tokio::test]
    #[ignore = "requires macOS pasteboard service; isolated pasteboard"]
    async fn concurrent_workers_share_a_named_pasteboard_safely() {
        let name = format!("nooboard.diagnostic.regression.{}", std::process::id());
        let a = crate::Clipboard::open_diagnostic(name.clone(), Duration::from_millis(10), 1024)
            .unwrap();
        let b = crate::Clipboard::open_diagnostic(name, Duration::from_millis(10), 1024).unwrap();
        for index in 0..250 {
            let (written, read) = tokio::join!(a.write_text(format!("text {index}")), b.read());
            written.unwrap();
            read.unwrap();
            let (written, read) = tokio::join!(b.write_text(format!("reply {index}")), a.read());
            written.unwrap();
            read.unwrap();
        }
        a.write_text(String::new()).await.unwrap();
        b.shutdown().await.unwrap();
        a.shutdown().await.unwrap();
    }
}
