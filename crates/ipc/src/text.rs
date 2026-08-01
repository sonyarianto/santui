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

/// Split text into lines that fit `max_width` (character-based), keeping whole words.
pub fn word_wrap(text: &str, max_width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        if line.len() + word.len() + 1 > max_width && !line.is_empty() {
            lines.push(line.clone());
            line.clear();
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() {
        lines.push(line);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

/// Parse a comma-separated tag list into sorted, deduplicated lowercase tags.
pub fn parse_tags(input: &str) -> Vec<String> {
    let mut tags: Vec<String> = input
        .split(',')
        .map(str::trim)
        .filter(|tag| !tag.is_empty())
        .map(|tag| tag.to_lowercase())
        .collect();
    tags.sort();
    tags.dedup();
    tags
}

/// Single-line preview of a value: newlines become `⏎`, truncated to 90 chars.
pub fn single_line(value: &str) -> String {
    crate::ui::truncate(&value.replace('\n', " ⏎ "), 90)
}
