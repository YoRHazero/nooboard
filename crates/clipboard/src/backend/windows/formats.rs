use super::{handles::ClipboardGuard, native_error};
use crate::backend::PreparedWrite;
use crate::{
    Error, ImageData, ImageEncoding, Limits, Payload, ReadState, Result, SkipReason,
    formats::{self as common, Kind},
};
use windows_sys::{
    Win32::{
        System::{DataExchange::*, Memory::*},
        UI::Shell::DragQueryFileW,
    },
    core::w,
};
const TEXT: u32 = 13;
pub(crate) fn dib_v5(image: &ImageData) -> Result<Vec<u8>> {
    let pixels = image.decode()?.to_rgba8();
    let mut bytes = vec![0u8; 124 + pixels.len()];
    bytes[0..4].copy_from_slice(&124u32.to_le_bytes());
    bytes[4..8].copy_from_slice(&(pixels.width() as i32).to_le_bytes());
    bytes[8..12].copy_from_slice(&(-(pixels.height() as i32)).to_le_bytes());
    bytes[12..14].copy_from_slice(&1u16.to_le_bytes());
    bytes[14..16].copy_from_slice(&32u16.to_le_bytes());
    bytes[16..20].copy_from_slice(&3u32.to_le_bytes());
    bytes[20..24].copy_from_slice(&(pixels.len() as u32).to_le_bytes());
    for (offset, mask) in [
        (40, 0x00ff0000u32),
        (44, 0x0000ff00),
        (48, 0x000000ff),
        (52, 0xff000000),
        (56, 0x73524742),
    ] {
        bytes[offset..offset + 4].copy_from_slice(&mask.to_le_bytes());
    }
    for (input, output) in pixels
        .as_raw()
        .chunks_exact(4)
        .zip(bytes[124..].chunks_exact_mut(4))
    {
        output.copy_from_slice(&[input[2], input[1], input[0], input[3]]);
    }
    Ok(bytes)
}
pub(crate) fn from_dib(dib: &[u8]) -> Result<ImageData> {
    if dib.len() < 40 || dib.len() > 256 * 1024 * 1024 + 124 {
        return Err(Error::InvalidInput);
    }
    let u32_at = |p| u32::from_le_bytes(dib[p..p + 4].try_into().unwrap());
    let header = u32_at(0) as usize;
    if ![40, 52, 56, 108, 124].contains(&header) || header > dib.len() {
        return Err(Error::InvalidInput);
    }
    let bits = u16::from_le_bytes(dib[14..16].try_into().unwrap());
    let compression = u32_at(16);
    let colors = if u32_at(32) != 0 {
        u32_at(32) as usize
    } else if bits <= 8 {
        1usize << bits
    } else {
        0
    };
    let masks = if header == 40 && compression == 3 {
        12
    } else if header == 40 && compression == 6 {
        16
    } else {
        0
    };
    let offset = header
        .checked_add(masks)
        .and_then(|n| colors.checked_mul(4).and_then(|c| n.checked_add(c)))
        .ok_or(Error::InvalidInput)?;
    if offset > dib.len() {
        return Err(Error::InvalidInput);
    }
    let mut bmp = Vec::with_capacity(dib.len() + 14);
    bmp.extend_from_slice(b"BM");
    bmp.extend_from_slice(&((dib.len() + 14) as u32).to_le_bytes());
    bmp.extend_from_slice(&[0; 4]);
    bmp.extend_from_slice(&((offset + 14) as u32).to_le_bytes());
    bmp.extend_from_slice(dib);
    // This native allocation bound is separate from the service's image_bytes
    // limit, which the runtime also checks on the wrapped BMP before publication.
    ImageData::from_native_bmp(bmp)
}
fn registered(name: *const u16) -> Result<u32> {
    // SAFETY: callers supply static NUL-terminated format names.
    let id = unsafe { RegisterClipboardFormatW(name) };
    if id == 0 {
        Err(native_error("register clipboard format"))
    } else {
        Ok(id)
    }
}
pub(super) fn read(guard: &ClipboardGuard, limits: &Limits) -> Result<ReadState> {
    let known = [
        (
            registered(w!("ExcludeClipboardContentFromMonitorProcessing"))?,
            Kind::Sensitive,
        ),
        (15, Kind::Files),
        (
            registered(w!("FileGroupDescriptorW"))?,
            Kind::UnsupportedFiles,
        ),
        (
            registered(w!("FileGroupDescriptor"))?,
            Kind::UnsupportedFiles,
        ),
        (registered(w!("FileContents"))?, Kind::UnsupportedFiles),
        (registered(w!("PNG"))?, Kind::Image(ImageEncoding::Png)),
        (17, Kind::Image(ImageEncoding::Bmp)),
        (8, Kind::Image(ImageEncoding::Bmp)),
        (registered(w!("JFIF"))?, Kind::Image(ImageEncoding::Jpeg)),
        (2, Kind::UnsupportedImage),
        (TEXT, Kind::Text),
    ];
    // SAFETY: the clipboard is open while guard is borrowed.
    let mut offered: Vec<_> = known
        .into_iter()
        .filter(|(id, _)| unsafe { IsClipboardFormatAvailable(*id) != 0 })
        .collect();
    // SAFETY: caller holds the clipboard guard.
    if offered.is_empty() && unsafe { CountClipboardFormats() } != 0 {
        offered.push((0, Kind::Other));
    }
    let plan = common::select(offered);
    let Some(candidate) = plan.candidates.first() else {
        // SAFETY: clipboard is open on this thread.
        return Ok(if plan.empty && unsafe { CountClipboardFormats() } != 0 {
            ReadState::Skipped(SkipReason::Unsupported)
        } else {
            plan.fallback()
        });
    };
    match candidate.kind {
        Kind::Files => read_files(guard, limits),
        Kind::Image(encoding) => {
            let dib = candidate.format == 17 || candidate.format == 8;
            let limit = if dib {
                256 * 1024 * 1024 + 124
            } else {
                limits.image_bytes
            };
            Ok(match read_bytes(guard, candidate.format, limit)? {
                Some(bytes) => ReadState::Ready(Payload::Image(if dib {
                    from_dib(&bytes)?
                } else {
                    ImageData::new(encoding, bytes)?
                })),
                None => ReadState::Skipped(SkipReason::TooLarge),
            })
        }
        Kind::Text => {
            let Some(bytes) = read_bytes(
                guard,
                TEXT,
                limits.text_bytes.saturating_mul(2).saturating_add(2),
            )?
            else {
                return Ok(ReadState::Skipped(SkipReason::TooLarge));
            };
            if !bytes.len().is_multiple_of(2) {
                return Err(Error::InvalidData);
            }
            let words: Vec<_> = bytes
                .chunks_exact(2)
                .map(|b| u16::from_le_bytes([b[0], b[1]]))
                .collect();
            let end = words
                .iter()
                .position(|&c| c == 0)
                .ok_or(Error::InvalidData)?;
            let text = String::from_utf16(&words[..end]).map_err(|_| Error::InvalidData)?;
            Ok(ReadState::Ready(Payload::Text(text)))
        }
        _ => unreachable!("selection only returns readable formats"),
    }
}
fn read_files(_: &ClipboardGuard, limits: &Limits) -> Result<ReadState> {
    use std::os::windows::ffi::OsStringExt;
    // SAFETY: clipboard guard is live, the handle is borrowed, and DragQueryFileW
    // writes into a sized UTF-16 buffer including the terminator.
    unsafe {
        let handle = GetClipboardData(15);
        if handle.is_null() {
            return Err(Error::Unavailable);
        }
        let count = DragQueryFileW(handle, u32::MAX, std::ptr::null_mut(), 0);
        if count == 0 {
            return Err(Error::InvalidData);
        }
        if count as usize > limits.files {
            return Ok(ReadState::Skipped(SkipReason::TooLarge));
        }
        let mut files = Vec::new();
        let mut total = 0usize;
        for index in 0..count {
            let len = DragQueryFileW(handle, index, std::ptr::null_mut(), 0);
            if len == 0 || len > 32767 {
                return Err(Error::InvalidData);
            }
            total = total.saturating_add(len as usize * 2);
            if total > limits.file_list_bytes {
                return Ok(ReadState::Skipped(SkipReason::TooLarge));
            }
            let mut path = vec![0u16; len as usize + 1];
            if DragQueryFileW(handle, index, path.as_mut_ptr(), len + 1) != len {
                return Err(native_error("read clipboard file path"));
            }
            files.push(std::path::PathBuf::from(std::ffi::OsString::from_wide(
                &path[..len as usize],
            )));
        }
        Ok(ReadState::Ready(Payload::Files(files)))
    }
}
fn read_bytes(_: &ClipboardGuard, format: u32, limit: usize) -> Result<Option<Vec<u8>>> {
    // SAFETY: a live clipboard guard is required. Data is bounded by GlobalSize
    // and copied while locked; the borrowed system handle is never freed here.
    unsafe {
        let handle = GetClipboardData(format);
        if handle.is_null() {
            return Err(Error::Unavailable);
        }
        let length = GlobalSize(handle);
        if length > limit {
            return Ok(None);
        }
        if length == 0 {
            return Err(Error::InvalidData);
        }
        let pointer = GlobalLock(handle).cast::<u8>();
        if pointer.is_null() {
            return Err(native_error("read clipboard memory"));
        }
        let bytes = std::slice::from_raw_parts(pointer, length).to_vec();
        GlobalUnlock(handle);
        Ok(Some(bytes))
    }
}
pub(super) fn encode(prepared: &mut PreparedWrite) -> Result<Vec<(u32, Vec<u8>)>> {
    Ok(match &prepared.payload {
        Payload::Text(text) => vec![(
            TEXT,
            text.encode_utf16()
                .chain(Some(0))
                .flat_map(u16::to_le_bytes)
                .collect(),
        )],
        Payload::Image(image) => vec![
            (registered(w!("PNG"))?, image.bytes().to_vec()),
            (17, prepared.dib.take().ok_or(Error::InvalidInput)?),
        ],
        Payload::Files(files) => {
            use std::os::windows::ffi::OsStrExt;
            let mut data = vec![0u8; 20];
            data[0..4].copy_from_slice(&20u32.to_le_bytes());
            data[16..20].copy_from_slice(&1u32.to_le_bytes());
            for path in files {
                for word in path.as_os_str().encode_wide() {
                    data.extend_from_slice(&word.to_le_bytes());
                }
                data.extend_from_slice(&[0, 0]);
            }
            data.extend_from_slice(&[0, 0]);
            vec![
                (15, data),
                (
                    registered(w!("Preferred DropEffect"))?,
                    1u32.to_le_bytes().to_vec(),
                ),
            ]
        }
    })
}
