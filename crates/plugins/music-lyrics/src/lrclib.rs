fn de_f64_lenient<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de;
    struct F64Visitor;
    impl<'de> de::Visitor<'de> for F64Visitor {
        type Value = f64;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("a number (integer or float)")
        }
        fn visit_f64<E: de::Error>(self, v: f64) -> Result<f64, E> {
            Ok(v)
        }
        fn visit_i64<E: de::Error>(self, v: i64) -> Result<f64, E> {
            Ok(v as f64)
        }
        fn visit_u64<E: de::Error>(self, v: u64) -> Result<f64, E> {
            Ok(v as f64)
        }
    }
    deserializer.deserialize_any(F64Visitor)
}

use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct LyricsData {
    pub text: String,
    pub source: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct LRCLibTrack {
    pub id: u64,
    pub track_name: String,
    pub artist_name: String,
    pub album_name: String,
    #[serde(deserialize_with = "de_f64_lenient")]
    pub duration: f64,
    #[serde(default)]
    pub instrumental: bool,
    #[serde(default)]
    pub plain_lyrics: Option<String>,
    #[serde(default)]
    pub synced_lyrics: Option<String>,
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

pub fn search(query: &str) -> Result<Vec<LRCLibTrack>, String> {
    let encoded = url_encode(query);
    let url = format!("https://lrclib.net/api/search?q={}", encoded);

    let mut resp = ureq::get(&url)
        .call()
        .map_err(|e| format!("LRCLib request failed: {e}"))?;
    let body: String = resp
        .body_mut()
        .read_to_string()
        .map_err(|e| format!("LRCLib read body failed: {e}"))?;

    let tracks: Vec<LRCLibTrack> =
        serde_json::from_str(&body).map_err(|e| format!("LRCLib parse failed: {e}"))?;

    Ok(tracks)
}

pub fn extract_lyrics(track: &LRCLibTrack) -> Option<LyricsData> {
    if track.instrumental {
        return None;
    }
    if let Some(ref plain) = track.plain_lyrics {
        if !plain.is_empty() {
            return Some(LyricsData {
                text: plain.clone(),
                source: "LRCLib".into(),
            });
        }
    }
    if let Some(ref synced) = track.synced_lyrics {
        if !synced.is_empty() {
            let text = strip_timestamps(synced);
            if !text.is_empty() {
                return Some(LyricsData {
                    text,
                    source: "LRCLib".into(),
                });
            }
        }
    }
    None
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
        assert_eq!(url_encode("caf\u{e9}"), "caf%C3%A9");
    }

    #[test]
    fn parse_lrclib_response() {
        let json = r#"[
            {
                "id": 12345,
                "trackName": "Lose Yourself",
                "artistName": "Eminem",
                "albumName": "8 Mile Soundtrack",
                "duration": 326,
                "instrumental": false,
                "plainLyrics": "His palms are sweaty\nKnees weak\nMom's spaghetti"
            }
        ]"#;
        let tracks: Vec<LRCLibTrack> = serde_json::from_str(json).unwrap();
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].id, 12345);
        assert_eq!(tracks[0].track_name, "Lose Yourself");
        assert_eq!(tracks[0].artist_name, "Eminem");
        assert_eq!(tracks[0].album_name, "8 Mile Soundtrack");
        assert!((tracks[0].duration - 326.0).abs() < f64::EPSILON);
        assert!(!tracks[0].instrumental);
        assert_eq!(
            tracks[0].plain_lyrics,
            Some("His palms are sweaty\nKnees weak\nMom's spaghetti".into())
        );
    }

    #[test]
    fn extract_lyrics_from_plain_lyrics() {
        let track = LRCLibTrack {
            id: 1,
            track_name: "Test".into(),
            artist_name: "A".into(),
            album_name: "B".into(),
            duration: 100.0,
            instrumental: false,
            plain_lyrics: Some("line one\nline two".into()),
            synced_lyrics: None,
        };
        let lyrics = extract_lyrics(&track);
        assert!(lyrics.is_some());
        assert_eq!(lyrics.unwrap().text, "line one\nline two");
    }

    #[test]
    fn extract_lyrics_from_synced_lyrics() {
        let track = LRCLibTrack {
            id: 1,
            track_name: "Test".into(),
            artist_name: "A".into(),
            album_name: "B".into(),
            duration: 100.0,
            instrumental: false,
            plain_lyrics: None,
            synced_lyrics: Some("[00:01.00]line one\n[00:02.00]line two".into()),
        };
        let lyrics = extract_lyrics(&track);
        assert!(lyrics.is_some());
        assert_eq!(lyrics.unwrap().text, "line one\nline two");
    }

    #[test]
    fn extract_lyrics_instrumental_returns_none() {
        let track = LRCLibTrack {
            id: 1,
            track_name: "Test".into(),
            artist_name: "A".into(),
            album_name: "B".into(),
            duration: 100.0,
            instrumental: true,
            plain_lyrics: Some("noise".into()),
            synced_lyrics: None,
        };
        assert!(extract_lyrics(&track).is_none());
    }

    #[test]
    fn extract_lyrics_empty_returns_none() {
        let track = LRCLibTrack {
            id: 1,
            track_name: "Test".into(),
            artist_name: "A".into(),
            album_name: "B".into(),
            duration: 100.0,
            instrumental: false,
            plain_lyrics: None,
            synced_lyrics: None,
        };
        assert!(extract_lyrics(&track).is_none());
    }

    #[test]
    fn strip_timestamps_removes_brackets() {
        let input = "[00:01.00]hello\n[00:02.50]world";
        let result = strip_timestamps(input);
        assert_eq!(result, "hello\nworld");
    }

    #[test]
    fn strip_timestamps_skips_empty_lines() {
        let input = "[00:01.00]hello\n[00:02.00]\n[00:03.00]world";
        let result = strip_timestamps(input);
        assert_eq!(result, "hello\nworld");
    }

    #[test]
    fn strip_timestamps_empty_lyric_line_removed() {
        let input = "[00:01.00]  \n[00:02.00]text";
        let result = strip_timestamps(input);
        assert_eq!(result, "text");
    }

    #[test]
    fn parse_lrclib_empty_array() {
        let tracks: Vec<LRCLibTrack> = serde_json::from_str("[]").unwrap();
        assert!(tracks.is_empty());
    }

    #[test]
    fn lrclib_track_defaults_instrumental_false() {
        let json = r#"{"id":1,"trackName":"T","artistName":"A","albumName":"B","duration":100}"#;
        let track: LRCLibTrack = serde_json::from_str(json).unwrap();
        assert!(!track.instrumental);
        assert!(track.plain_lyrics.is_none());
        assert!(track.synced_lyrics.is_none());
    }
}
