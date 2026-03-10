use time::{OffsetDateTime, UtcOffset};

pub(crate) fn clock_label_from_millis(timestamp_ms: i64) -> String {
    let offset = UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC);
    clock_label_from_millis_with_offset(timestamp_ms, offset)
}

fn clock_label_from_millis_with_offset(timestamp_ms: i64, offset: UtcOffset) -> String {
    let nanos = i128::from(timestamp_ms) * 1_000_000;
    let datetime = OffsetDateTime::from_unix_timestamp_nanos(nanos)
        .unwrap_or(OffsetDateTime::UNIX_EPOCH)
        .to_offset(offset);

    format!(
        "{:02}:{:02}:{:02}",
        datetime.hour(),
        datetime.minute(),
        datetime.second()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_offset_clock_label_uses_local_offset() {
        assert_eq!(
            clock_label_from_millis_with_offset(0, UtcOffset::from_hms(9, 0, 0).unwrap()),
            "09:00:00"
        );
        assert_eq!(
            clock_label_from_millis_with_offset(3_723_000, UtcOffset::from_hms(-5, 0, 0).unwrap()),
            "20:02:03"
        );
    }
}
