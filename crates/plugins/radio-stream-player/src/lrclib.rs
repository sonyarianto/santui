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

    let mut resp = ureq::get(&url)
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
