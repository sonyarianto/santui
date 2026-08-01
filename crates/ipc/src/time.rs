/// Seconds since the UNIX epoch (UTC).
pub fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Local timestamp in ISO 8601 format, e.g. `2026-08-01T12:34:56+07:00`.
#[cfg(feature = "time")]
pub fn now_string() -> String {
    chrono::Local::now()
        .format("%Y-%m-%dT%H:%M:%S%:z")
        .to_string()
}
