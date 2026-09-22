use super::{ImmediateNative, Observation, Wake};
use crate::backend::PreparedWrite;
use crate::{
    Error, ImageData, ImageEncoding, Options, Origin, Payload, ReadState, Result, SkipReason,
    formats::{self, Kind, file_urls},
};
use objc2::{
    ClassType, msg_send,
    rc::{Retained, autoreleasepool},
    runtime::ProtocolObject,
};
use objc2_app_kit::{NSPasteboard, NSPasteboardItem, NSPasteboardWriting};
use objc2_foundation::{NSArray, NSData, NSString};
use std::{
    sync::{Mutex, MutexGuard},
    time::Duration,
};

// AppKit may return the same NSPasteboard object to separate clients in this process.
static PASTEBOARD_ACCESS: Mutex<()> = Mutex::new(());
fn access() -> MutexGuard<'static, ()> {
    PASTEBOARD_ACCESS.lock().unwrap_or_else(|e| e.into_inner())
}

pub(super) struct Native {
    board: Retained<NSPasteboard>,
    limits: crate::Limits,
}
impl Native {
    pub fn open(options: &Options) -> Result<Self> {
        let _access = access();
        #[cfg(feature = "diagnostics")]
        if let Some(name) = &options.diagnostic_name {
            // SAFETY: options validated the private diagnostic name; nullable class method.
            let board: Option<Retained<NSPasteboard>> = unsafe {
                msg_send![NSPasteboard::class(), pasteboardWithName: &*NSString::from_str(name)]
            };
            return Ok(Self {
                board: board.ok_or(Error::Unavailable)?,
                limits: options.limits.clone(),
            });
        }
        // SAFETY: documented nullable class method; the object stays on this thread.
        let board: Option<Retained<NSPasteboard>> =
            unsafe { msg_send![NSPasteboard::class(), generalPasteboard] };
        Ok(Self {
            board: board.ok_or(Error::Unavailable)?,
            limits: options.limits.clone(),
        })
    }
    fn token(&self) -> u64 {
        self.board.changeCount() as u64
    }
    fn read_inner(&self) -> Result<Observation> {
        autoreleasepool(|_| {
            let revision = self.token();
            let mut types: Vec<String> = self
                .board
                .types()
                .map(|v| v.iter().map(|s| s.to_string()).collect())
                .unwrap_or_default();
            types.sort_by_key(|name| match name.as_str() {
                "public.png" => 0,
                "public.tiff" => 1,
                "public.jpeg" => 2,
                "com.microsoft.bmp" => 3,
                _ => 4,
            });
            let plan = formats::select(types.iter().map(|name| {
                (
                    name.as_str(),
                    match name.as_str() {
                        "org.nspasteboard.ConcealedType" | "org.nspasteboard.TransientType" => {
                            Kind::Sensitive
                        }
                        "public.file-url" => Kind::Files,
                        "NSFilenamesPboardType"
                        | "com.apple.pasteboard.promised-file-url"
                        | "com.apple.pasteboard.promised-file-content-type" => {
                            Kind::UnsupportedFiles
                        }
                        "public.png" => Kind::Image(ImageEncoding::Png),
                        "public.tiff" => Kind::Image(ImageEncoding::Tiff),
                        "public.jpeg" => Kind::Image(ImageEncoding::Jpeg),
                        "com.microsoft.bmp" => Kind::Image(ImageEncoding::Bmp),
                        "public.heic" | "com.compuserve.gif" => Kind::UnsupportedImage,
                        "public.utf8-plain-text" => Kind::Text,
                        _ => Kind::Other,
                    },
                )
            }));
            let content = match plan.candidates.first() {
                None => plan.fallback(),
                Some(candidate) => match candidate.kind {
                    Kind::Files => self.read_files()?,
                    Kind::Image(encoding) => {
                        let data = self
                            .board
                            .dataForType(&NSString::from_str(candidate.format))
                            .ok_or(Error::Unavailable)?;
                        if data.len() > self.limits.image_bytes {
                            ReadState::Skipped(SkipReason::TooLarge)
                        } else {
                            ReadState::Ready(Payload::Image(ImageData::new(
                                encoding,
                                data.to_vec(),
                            )?))
                        }
                    }
                    Kind::Text => {
                        let text = self
                            .board
                            .stringForType(&NSString::from_str(candidate.format))
                            .ok_or(Error::Unavailable)?;
                        if text.len() > self.limits.text_bytes {
                            ReadState::Skipped(SkipReason::TooLarge)
                        } else {
                            ReadState::Ready(Payload::Text(text.to_string()))
                        }
                    }
                    _ => unreachable!("selection only returns readable formats"),
                },
            };
            if self.token() != revision {
                return Err(Error::Changed);
            }
            Ok(Observation {
                revision,
                content,
                origin: Origin::External,
            })
        })
    }
    fn read_files(&self) -> Result<ReadState> {
        let mut files = Vec::new();
        let mut bytes = 0usize;
        if let Some(items) = self.board.pasteboardItems() {
            for item in items {
                if let Some(value) = item.stringForType(&NSString::from_str("public.file-url")) {
                    bytes = bytes.saturating_add(value.len());
                    if bytes > self.limits.file_list_bytes {
                        return Ok(ReadState::Skipped(SkipReason::TooLarge));
                    }
                    match file_urls::parse(&value.to_string()) {
                        Ok(paths) => files.extend(paths),
                        Err(_) => return Ok(ReadState::Skipped(SkipReason::Unsupported)),
                    }
                }
                if files.len() > self.limits.files {
                    return Ok(ReadState::Skipped(SkipReason::TooLarge));
                }
            }
        }
        Ok(if files.is_empty() {
            ReadState::Skipped(SkipReason::Unsupported)
        } else {
            ReadState::Ready(Payload::Files(files))
        })
    }
}
impl ImmediateNative for Native {
    fn revision(&self) -> u64 {
        let _access = access();
        self.token()
    }
    fn wait(&mut self, wake: &Wake, timeout: Duration) -> Result<()> {
        wake.wait(timeout);
        Ok(())
    }
    fn read(&mut self) -> Result<Observation> {
        let _access = access();
        self.read_inner()
    }
    fn write(&mut self, prepared: PreparedWrite) -> Result<Observation> {
        let _access = access();
        let payload = prepared.payload;
        autoreleasepool(|_| {
            let items: Vec<Retained<ProtocolObject<dyn NSPasteboardWriting>>> = match &payload {
                Payload::Text(text) => {
                    let item = NSPasteboardItem::new();
                    if !item.setString_forType(
                        &NSString::from_str(text),
                        &NSString::from_str("public.utf8-plain-text"),
                    ) {
                        return Err(Error::backend(
                            "prepare pasteboard text",
                            "setString failed",
                        ));
                    }
                    vec![ProtocolObject::from_retained(item)]
                }
                Payload::Image(image) => {
                    let item = NSPasteboardItem::new();
                    if !item.setData_forType(
                        &NSData::with_bytes(image.bytes()),
                        &NSString::from_str("public.png"),
                    ) {
                        return Err(Error::backend("prepare pasteboard image", "setData failed"));
                    }
                    vec![ProtocolObject::from_retained(item)]
                }
                Payload::Files(files) => {
                    let urls = file_urls::encode(files)?;
                    let mut items = Vec::new();
                    for url in urls.lines() {
                        let item = NSPasteboardItem::new();
                        if !item.setString_forType(
                            &NSString::from_str(url),
                            &NSString::from_str("public.file-url"),
                        ) {
                            return Err(Error::backend(
                                "prepare pasteboard file",
                                "setString failed",
                            ));
                        }
                        items.push(ProtocolObject::from_retained(item));
                    }
                    items
                }
            };
            self.board.clearContents();
            if !self
                .board
                .writeObjects(&NSArray::from_retained_slice(&items))
            {
                return Err(Error::backend("publish pasteboard", "writeObjects failed"));
            }
            let mut observation = self.read_inner()?;
            if observation.content != ReadState::Ready(payload) {
                return Err(Error::Changed);
            }
            observation.origin = Origin::Application;
            Ok(observation)
        })
    }
}
