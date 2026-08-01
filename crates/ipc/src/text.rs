/// Percent-encode a string for use in a URL query (spaces become `+`).
pub fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for byte in s.bytes() {
        match byte {
            b' ' => out.push('+'),
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => {
                out.push_str(&format!("%{:02X}", byte));
            }
        }
    }
    out
}

/// Remove leading `[timestamp]`-style tags from each lyric line and drop empty lines.
pub fn strip_timestamps(lyrics: &str) -> String {
    lyrics
        .lines()
        .filter_map(|line| {
            let mut s = line;
            loop {
                let trimmed = s.trim_start();
                if trimmed.starts_with('[') {
                    if let Some(end) = trimmed.find(']') {
                        s = trimmed[end + 1..].trim_start();
                        continue;
                    }
                }
                break;
            }
            let result = s.trim();
            if result.is_empty() {
                None
            } else {
                Some(result.to_string())
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}
