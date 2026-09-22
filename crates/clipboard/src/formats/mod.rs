#[cfg(any(target_os = "macos", target_os = "linux", test))]
pub(crate) mod file_urls;
pub(crate) mod image;
use crate::{Error, ImageEncoding, Limits, Payload, ReadState, Result, SkipReason};

/// Native adapters describe formats; this module decides precedence and fallback.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Sensitive,
    Files,
    UnsupportedFiles,
    Image(ImageEncoding),
    UnsupportedImage,
    Text,
    Other,
}
pub(crate) struct Candidate<T> {
    pub format: T,
    pub kind: Kind,
}
pub(crate) struct ReadPlan<T> {
    pub candidates: Vec<Candidate<T>>,
    pub empty: bool,
    pub skipped: Option<SkipReason>,
}
pub(crate) fn select<T>(offers: impl IntoIterator<Item = (T, Kind)>) -> ReadPlan<T> {
    let mut files = None;
    let mut image = None;
    let mut text = None;
    let mut sensitive = false;
    let mut unsupported_files = false;
    let mut unsupported_image = false;
    let mut empty = true;
    for (format, kind) in offers {
        empty = false;
        match kind {
            Kind::Sensitive => sensitive = true,
            Kind::Files if files.is_none() => files = Some(Candidate { format, kind }),
            Kind::Image(_) if image.is_none() => image = Some(Candidate { format, kind }),
            Kind::Text if text.is_none() => text = Some(Candidate { format, kind }),
            Kind::UnsupportedFiles => unsupported_files = true,
            Kind::UnsupportedImage => unsupported_image = true,
            _ => {}
        }
    }
    let skipped = if sensitive {
        Some(SkipReason::Sensitive)
    } else if unsupported_files && files.is_none() {
        Some(SkipReason::Unsupported)
    } else {
        None
    };
    let mut candidates = Vec::new();
    if skipped.is_none() {
        if let Some(files) = files {
            candidates.push(files);
        }
        if let Some(image) = image {
            candidates.push(image);
        }
        // Never turn an image's descriptive text into clipboard text. A file URL
        // offer may fall through ONLY when its parser identifies non-file metadata.
        else if !unsupported_image && let Some(text) = text {
            candidates.push(text);
        }
    }
    ReadPlan {
        candidates,
        empty,
        skipped,
    }
}
impl<T> ReadPlan<T> {
    pub fn fallback(&self) -> ReadState {
        if self.empty {
            ReadState::Empty
        } else {
            ReadState::Skipped(self.skipped.unwrap_or(SkipReason::Unsupported))
        }
    }
}

pub(crate) fn validate(payload: &Payload, limits: &Limits) -> Result<()> {
    match payload {
        Payload::Text(text) if text.len() > limits.text_bytes || text.contains('\0') => {
            Err(Error::InvalidInput)
        }
        Payload::Image(image) if image.bytes().len() > limits.image_bytes => {
            Err(Error::InvalidInput)
        }
        Payload::Files(files) => {
            if files.is_empty()
                || files.len() > limits.files
                || files
                    .iter()
                    .any(|p| !p.is_absolute() || p.as_os_str().as_encoded_bytes().contains(&0))
                || files
                    .iter()
                    .try_fold(0usize, |total, p| {
                        total.checked_add(p.as_os_str().as_encoded_bytes().len())
                    })
                    .is_none_or(|n| n > limits.file_list_bytes)
            {
                Err(Error::InvalidInput)
            } else {
                Ok(())
            }
        }
        _ => Ok(()),
    }
}
pub(crate) fn normalize(payload: Payload, limits: &Limits) -> Result<Payload> {
    validate(&payload, limits)?;
    let payload = match payload {
        Payload::Image(image) => Payload::Image(image.png()?),
        other => other,
    };
    validate(&payload, limits)?;
    Ok(payload)
}
pub(crate) fn bound_observation(state: &mut ReadState, limits: &Limits) {
    if let ReadState::Ready(payload) = state
        && validate(payload, limits).is_err()
    {
        let reason = if matches!(payload, Payload::Text(text) if text.contains('\0')) {
            SkipReason::InvalidData
        } else {
            SkipReason::TooLarge
        };
        *state = ReadState::Skipped(reason);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sensitive_and_unsupported_rich_formats_prevent_text_fallback() {
        for kind in [
            Kind::Sensitive,
            Kind::UnsupportedFiles,
            Kind::UnsupportedImage,
        ] {
            let plan = select([(1, Kind::Text), (2, kind)]);
            assert!(plan.candidates.is_empty());
            assert!(matches!(plan.fallback(), ReadState::Skipped(_)));
        }
        let plan = select([
            (1, Kind::Text),
            (2, Kind::Image(ImageEncoding::Png)),
            (3, Kind::Files),
        ]);
        assert_eq!(
            plan.candidates.iter().map(|c| c.format).collect::<Vec<_>>(),
            vec![3, 2]
        );
    }
}
