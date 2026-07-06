use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct LyricsData {
    pub text: String,
    pub source: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LRCLibItem {
    #[serde(default)]
    plain_lyrics: Option<String>,
    #[serde(default)]
    synced_lyrics: Option<String>,
    #[serde(default)]
    instrumental: Option<bool>,
}

fn url_encode(s: &str) -> String {
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

/// Split a stream title like "Artist - Song" into (artist, title).
pub fn split_title(title: &str) -> (Option<String>, String) {
    if let Some(idx) = title.find(" - ") {
        let artist = title[..idx].trim();
        let track = title[idx + 3..].trim();
        if !artist.is_empty() && !track.is_empty() {
            return (Some(artist.to_string()), track.to_string());
        }
    }
    (None, title.to_string())
}

/// Fetch lyrics from LRCLib. Returns `None` if no lyrics found.
pub fn fetch(title: &str, artist: Option<&str>) -> Result<Option<LyricsData>, String> {
    let encoded_title = url_encode(title);
    let mut url = format!("https://lrclib.net/api/search?track_name={}", encoded_title);
    if let Some(artist) = artist {
        let encoded_artist = url_encode(artist);
        url.push_str("&artist_name=");
        url.push_str(&encoded_artist);
    }

    let mut resp = crate::http::agent()
        .get(&url)
        .call()
        .map_err(|e| format!("LRCLib request failed: {e}"))?;
    let body: String = resp
        .body_mut()
        .read_to_string()
        .map_err(|e| format!("LRCLib read body failed: {e}"))?;

    let items: Vec<LRCLibItem> =
        serde_json::from_str(&body).map_err(|e| format!("LRCLib parse failed: {e}"))?;

    for item in items {
        if item.instrumental.unwrap_or(false) {
            continue;
        }
        if let Some(ref plain) = item.plain_lyrics {
            if !plain.is_empty() {
                return Ok(Some(LyricsData {
                    text: plain.clone(),
                    source: "LRCLib".into(),
                }));
            }
        }
        if let Some(ref synced) = item.synced_lyrics {
            if !synced.is_empty() {
                let text = strip_timestamps(synced);
                if !text.is_empty() {
                    return Ok(Some(LyricsData {
                        text,
                        source: "LRCLib".into(),
                    }));
                }
            }
        }
    }

    Ok(None)
}

fn strip_timestamps(lyrics: &str) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_encode_alphanumeric() {
        assert_eq!(url_encode("hello123"), "hello123");
    }

    #[test]
    fn url_encode_space_to_plus() {
        assert_eq!(url_encode("hello world"), "hello+world");
    }

    #[test]
    fn url_encode_special_chars() {
        assert_eq!(url_encode("a&b=c/d"), "a%26b%3Dc%2Fd");
    }

    #[test]
    fn url_encode_safe_punctuation() {
        assert_eq!(url_encode("-_.~"), "-_.~");
    }

    #[test]
    fn url_encode_empty() {
        assert_eq!(url_encode(""), "");
    }

    #[test]
    fn url_encode_unicode() {
        assert_eq!(url_encode("café"), "caf%C3%A9");
    }

    #[test]
    fn split_title_artist_dash_track() {
        let (a, t) = split_title("Artist - Song");
        assert_eq!(a, Some("Artist".into()));
        assert_eq!(t, "Song");
    }

    #[test]
    fn split_title_no_dash() {
        let (a, t) = split_title("JustSong");
        assert_eq!(a, None);
        assert_eq!(t, "JustSong");
    }

    #[test]
    fn split_title_empty_artist_returns_none() {
        let (a, t) = split_title(" - Song");
        assert_eq!(a, None);
        assert_eq!(t, " - Song");
    }

    #[test]
    fn split_title_empty_track_returns_none() {
        let (a, t) = split_title("Artist - ");
        assert_eq!(a, None);
        assert_eq!(t, "Artist - ");
    }

    #[test]
    fn split_title_trims_whitespace() {
        let (a, t) = split_title("  Artist  -  Song  ");
        assert_eq!(a, Some("Artist".into()));
        assert_eq!(t, "Song");
    }

    #[test]
    fn strip_timestamps_simple() {
        assert_eq!(strip_timestamps("[00:12.34]Hello"), "Hello");
    }

    #[test]
    fn strip_timestamps_multiple_lines() {
        let input = "[00:01.00]Line one\n[00:02.00]Line two";
        assert_eq!(strip_timestamps(input), "Line one\nLine two");
    }

    #[test]
    fn strip_timestamps_empty_lyric_line_removed() {
        let input = "[00:01.00]Line one\n[00:02.00]\n[00:03.00]Line three";
        assert_eq!(strip_timestamps(input), "Line one\nLine three");
    }

    #[test]
    fn strip_timestamps_no_timestamps() {
        assert_eq!(strip_timestamps("Hello\nWorld"), "Hello\nWorld");
    }

    #[test]
    fn strip_timestamps_multiple_brackets() {
        assert_eq!(strip_timestamps("[re:00:01.00][ti:Title]Hello"), "Hello");
    }

    #[test]
    fn strip_timestamps_empty_input() {
        assert_eq!(strip_timestamps(""), "");
    }

    #[test]
    fn strip_timestamps_whitespace_only_line_removed() {
        let input = "[00:01.00]  \n[00:02.00]Hello";
        assert_eq!(strip_timestamps(input), "Hello");
    }
}
