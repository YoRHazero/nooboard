pub(crate) fn now_millis_i64() -> i64 {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);

    if millis > i64::MAX as u128 {
        i64::MAX
    } else {
        millis as i64
    }
}
