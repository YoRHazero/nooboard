use nooboard_platform::NooboardError;
use objc2::rc::{Retained, autoreleasepool};
use objc2::runtime::AnyObject;
use objc2::{ClassType, msg_send};
use objc2_app_kit::{NSPasteboard, NSPasteboardType, NSPasteboardTypeString};
use objc2_foundation::NSString;

#[inline]
fn utf8_plain_text_type() -> &'static NSPasteboardType {
    // SAFETY: This is an AppKit-provided immutable global constant for UTF-8 plain text UTI.
    unsafe { NSPasteboardTypeString }
}

fn general_pasteboard() -> Result<Retained<NSPasteboard>, NooboardError> {
    let class = NSPasteboard::class();
    let ptr: *mut AnyObject = {
        // SAFETY: We are sending the documented class method +[NSPasteboard generalPasteboard].
        unsafe { msg_send![class, generalPasteboard] }
    };

    // SAFETY: `ptr` either points to an ObjC object or is NULL. We check NULL via `Option`.
    let pasteboard = unsafe { Retained::retain(ptr.cast::<NSPasteboard>()) }.ok_or_else(|| {
        NooboardError::platform("NSPasteboard.generalPasteboard is unavailable in current session")
    })?;

    Ok(pasteboard)
}

pub(crate) fn read_text_from_pasteboard() -> Result<Option<String>, NooboardError> {
    autoreleasepool(|_| {
        let pasteboard = general_pasteboard()?;
        let text = pasteboard
            .stringForType(utf8_plain_text_type())
            .map(|value| value.to_string());
        Ok(text)
    })
}

pub(crate) fn write_text_to_pasteboard(text: &str) -> Result<(), NooboardError> {
    autoreleasepool(|_| {
        let pasteboard = general_pasteboard()?;
        let _ = pasteboard.clearContents();

        let ns_text = NSString::from_str(text);
        let written = pasteboard.setString_forType(&ns_text, utf8_plain_text_type());
        if written {
            Ok(())
        } else {
            Err(NooboardError::platform(
                "failed to write UTF-8 text into NSPasteboard",
            ))
        }
    })
}

pub(crate) fn pasteboard_change_count() -> Result<isize, NooboardError> {
    let pasteboard = general_pasteboard()?;
    Ok(pasteboard.changeCount())
}
