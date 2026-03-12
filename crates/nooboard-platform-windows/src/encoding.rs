use nooboard_platform::NooboardError;

pub(crate) fn encode_cf_unicode_text(text: &str) -> Result<Vec<u16>, NooboardError> {
    if text.contains('\0') {
        return Err(NooboardError::platform(
            "CF_UNICODETEXT does not support interior NUL characters",
        ));
    }

    Ok(text.encode_utf16().chain(std::iter::once(0)).collect())
}

pub(crate) fn decode_cf_unicode_text(units: &[u16]) -> Result<String, NooboardError> {
    let len = units
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(units.len());
    String::from_utf16(&units[..len]).map_err(|error| {
        NooboardError::platform(format!("clipboard text is not valid UTF-16: {error}"))
    })
}

#[cfg(test)]
mod tests {
    use super::{decode_cf_unicode_text, encode_cf_unicode_text};

    #[test]
    fn encode_appends_nul_terminator() {
        let encoded = encode_cf_unicode_text("hello").unwrap();
        assert_eq!(encoded, vec![104, 101, 108, 108, 111, 0]);
    }

    #[test]
    fn encode_rejects_interior_nul() {
        let error = encode_cf_unicode_text("hello\0world").unwrap_err();
        assert!(error.to_string().contains("interior NUL"));
    }

    #[test]
    fn decode_stops_at_first_nul() {
        let decoded = decode_cf_unicode_text(&[104, 105, 0, 120]).unwrap();
        assert_eq!(decoded, "hi");
    }

    #[test]
    fn decode_reports_invalid_utf16() {
        let error = decode_cf_unicode_text(&[0xD800]).unwrap_err();
        assert!(error.to_string().contains("valid UTF-16"));
    }
}
