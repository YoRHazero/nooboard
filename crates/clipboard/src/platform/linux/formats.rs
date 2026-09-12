use crate::Content;

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
