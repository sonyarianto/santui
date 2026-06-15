use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct TrackInfo {
    pub artist: Option<String>,
    pub title: Option<String>,
}

#[derive(Deserialize)]
struct ITunesResponse {
    #[serde(rename = "resultCount")]
    result_count: u32,
    results: Vec<ITunesTrack>,
}

#[derive(Deserialize)]
struct ITunesTrack {
    #[serde(rename = "trackName")]
    track_name: Option<String>,
    #[serde(rename = "artistName")]
    artist_name: Option<String>,
    #[allow(dead_code)]
    #[serde(rename = "collectionName")]
    collection_name: Option<String>,
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

pub fn lookup(title: &str) -> Result<Option<TrackInfo>, String> {
    let encoded = url_encode(title);
    let url = format!(
        "https://itunes.apple.com/search?term={}&entity=song&limit=1",
        encoded
    );

    let body: String = ureq::get(&url)
        .call()
        .map_err(|e| format!("iTunes request failed: {e}"))?
        .into_string()
        .map_err(|e| format!("iTunes read body failed: {e}"))?;

    let parsed: ITunesResponse =
        serde_json::from_str(&body).map_err(|e| format!("iTunes parse failed: {e}"))?;

    if parsed.result_count == 0 {
        return Ok(None);
    }

    let track = &parsed.results[0];
    Ok(Some(TrackInfo {
        artist: track.artist_name.clone(),
        title: track.track_name.clone(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_returns_michael_jackson() {
        let result = lookup("MICHAEL JACKSON - YOU ARE NOT ALONE");
        assert!(result.is_ok(), "lookup failed: {:?}", result.err());
        let info = result.unwrap();
        assert!(info.is_some(), "no results from iTunes");
        let info = info.unwrap();
        assert_eq!(info.artist.as_deref(), Some("Michael Jackson"));
        assert!(info.title.is_some());
    }
}
