use crate::{
    Error, ImageData, ImageEncoding, Limits, Payload as ClipboardPayload, ReadState, Result,
    SkipReason,
    formats::{self, Candidate, Kind, ReadPlan, file_urls},
};
use std::{collections::BTreeMap, sync::Arc};

pub(super) struct Payload {
    pub content: ClipboardPayload,
    pub data: BTreeMap<String, Arc<Vec<u8>>>,
}
impl Payload {
    pub fn new(content: ClipboardPayload) -> Result<Self> {
        let mut data = BTreeMap::new();
        match &content {
            ClipboardPayload::Text(text) => {
                let bytes = Arc::new(text.as_bytes().to_vec());
                for mime in UTF8_TYPES {
                    data.insert((*mime).into(), bytes.clone());
                }
                if bytes.is_ascii() {
                    data.insert("STRING".into(), bytes);
                }
            }
            ClipboardPayload::Image(image) => {
                data.insert("image/png".into(), image.shared_bytes());
            }
            ClipboardPayload::Files(files) => {
                let urls = file_urls::encode(files)?;
                data.insert("text/uri-list".into(), Arc::new(urls.as_bytes().to_vec()));
                data.insert(
                    "x-special/gnome-copied-files".into(),
                    Arc::new(format!("copy\n{}", urls.replace("\r\n", "\n")).into_bytes()),
                );
                data.insert(
                    "application/x-kde-cutselection".into(),
                    Arc::new(vec![b'0']),
                );
            }
        }
        Ok(Self { content, data })
    }
}
const FILE_TYPES: &[&str] = &["x-special/gnome-copied-files", "text/uri-list"];
const IMAGE_TYPES: &[&str] = &["image/png", "image/tiff", "image/jpeg", "image/bmp"];
const UTF8_TYPES: &[&str] = &[
    "text/plain;charset=utf-8",
    "text/plain;charset=UTF-8",
    "UTF8_STRING",
    "text/plain",
];

pub(super) fn plan(types: &[String]) -> ReadPlan<usize> {
    let mut indices: Vec<_> = (0..types.len()).collect();
    indices.sort_by_key(|&i| {
        FILE_TYPES
            .iter()
            .chain(IMAGE_TYPES)
            .chain(UTF8_TYPES)
            .position(|t| *t == types[i])
            .unwrap_or(usize::MAX)
    });
    formats::select(indices.into_iter().map(|i| {
        (
            i,
            match types[i].as_str() {
                "x-kde-passwordManagerHint" | "application/x-kde-passwordManagerHint" => {
                    Kind::Sensitive
                }
                "x-special/gnome-copied-files" | "text/uri-list" => Kind::Files,
                "application/x-kde-cutselection" => Kind::UnsupportedFiles,
                "image/png" => Kind::Image(ImageEncoding::Png),
                "image/tiff" => Kind::Image(ImageEncoding::Tiff),
                "image/jpeg" => Kind::Image(ImageEncoding::Jpeg),
                "image/bmp" => Kind::Image(ImageEncoding::Bmp),
                text if UTF8_TYPES.contains(&text) || text == "STRING" => Kind::Text,
                image if image.starts_with("image/") => Kind::UnsupportedImage,
                _ => Kind::Other,
            },
        )
    }))
}
pub(super) fn limit(candidate: &Candidate<usize>, limits: &Limits) -> usize {
    match candidate.kind {
        Kind::Files => limits.file_list_bytes,
        Kind::Image(_) => limits.image_bytes,
        _ => limits.text_bytes,
    }
}
/// None permits fallback only for URI metadata that contains no local file reference.
pub(super) fn decode(kind: Kind, mime: &str, bytes: Vec<u8>) -> Result<Option<ReadState>> {
    Ok(Some(match kind {
        Kind::Files => {
            let Ok(text) = std::str::from_utf8(&bytes) else {
                return Ok(Some(ReadState::Skipped(SkipReason::InvalidData)));
            };
            match file_urls::parse(text) {
                Ok(paths) => ReadState::Ready(ClipboardPayload::Files(paths)),
                Err(_)
                    if mime == "x-special/gnome-copied-files"
                        || text.lines().any(|l| l.trim().starts_with("file:")) =>
                {
                    ReadState::Skipped(SkipReason::Unsupported)
                }
                Err(_) => return Ok(None),
            }
        }
        Kind::Image(encoding) => {
            ReadState::Ready(ClipboardPayload::Image(ImageData::new(encoding, bytes)?))
        }
        Kind::Text => {
            let text = if mime == "STRING" {
                bytes.into_iter().map(char::from).collect()
            } else {
                String::from_utf8(bytes).map_err(|_| Error::InvalidData)?
            };
            if text.contains('\0') {
                ReadState::Skipped(SkipReason::InvalidData)
            } else {
                ReadState::Ready(ClipboardPayload::Text(text))
            }
        }
        _ => return Err(Error::InvalidData),
    }))
}
