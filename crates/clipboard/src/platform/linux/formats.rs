use crate::Content;
use crate::{Error, ImageData, ImageEncoding, Result, file_urls};
use std::{collections::BTreeMap, sync::Arc};

pub(super) struct Payload {
    pub content: Content,
    pub data: BTreeMap<String, Arc<Vec<u8>>>,
}
impl Payload {
    pub fn new(content: Content) -> Result<Self> {
        let mut data = BTreeMap::new();
        match &content {
            Content::Text(text) => {
                let bytes = Arc::new(text.as_bytes().to_vec());
                for mime in UTF8_TYPES {
                    data.insert((*mime).into(), bytes.clone());
                }
                if bytes.is_ascii() {
                    data.insert("STRING".into(), bytes);
                }
            }
            Content::Image(image) if image.encoding == ImageEncoding::Png => {
                data.insert("image/png".into(), image.bytes.clone());
            }
            Content::Files(files) => {
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
            _ => return Err(Error::InvalidInput),
        }
        Ok(Self { content, data })
    }
}

pub(super) const FILE_TYPES: &[&str] = &["x-special/gnome-copied-files", "text/uri-list"];
pub(super) const IMAGE_TYPES: &[&str] = &["image/png", "image/tiff", "image/jpeg", "image/bmp"];
pub(super) fn image(mime: &str, bytes: Vec<u8>) -> Result<Content> {
    let encoding = match mime {
        "image/png" => ImageEncoding::Png,
        "image/tiff" => ImageEncoding::Tiff,
        "image/jpeg" => ImageEncoding::Jpeg,
        "image/bmp" => ImageEncoding::Bmp,
        _ => return Err(Error::InvalidInput),
    };
    Ok(Content::Image(ImageData::new(encoding, bytes)?))
}
pub(super) fn files(mime: &str, bytes: &[u8]) -> Option<Content> {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return Some(Content::Unsupported);
    };
    match file_urls::parse(text) {
        Ok(paths) => Some(Content::Files(paths)),
        Err(_)
            if mime == "x-special/gnome-copied-files"
                || text.lines().any(|l| l.trim().starts_with("file:")) =>
        {
            Some(Content::Unsupported)
        }
        Err(_) => None, // Web URL metadata is not a local file reference.
    }
}

pub(super) fn excluded(types: &[String]) -> Option<Content> {
    if types.iter().any(|t| {
        matches!(
            t.as_str(),
            "x-kde-passwordManagerHint" | "application/x-kde-passwordManagerHint"
        )
    }) {
        Some(Content::Sensitive)
    } else if types.iter().any(|t| {
        t.starts_with("image/")
            || matches!(
                t.as_str(),
                "text/uri-list" | "x-special/gnome-copied-files" | "application/x-kde-cutselection"
            )
    }) {
        Some(Content::Unsupported)
    } else {
        None
    }
}
pub(super) const UTF8_TYPES: &[&str] = &[
    "text/plain;charset=utf-8",
    "text/plain;charset=UTF-8",
    "UTF8_STRING",
    "text/plain",
];

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nontext_and_sensitive_formats_win_over_text_fallbacks() {
        for format in ["image/png", "text/uri-list", "x-special/gnome-copied-files"] {
            assert_eq!(
                excluded(&["text/plain".into(), format.into()]),
                Some(Content::Unsupported)
            );
        }
        assert_eq!(
            excluded(&["text/plain".into(), "x-kde-passwordManagerHint".into()]),
            Some(Content::Sensitive)
        );
        assert_eq!(excluded(&["text/plain".into()]), None);
    }
}
