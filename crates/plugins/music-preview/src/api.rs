use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItunesResponse {
    #[serde(rename = "resultCount")]
    pub result_count: u32,
    pub results: Vec<ItunesTrack>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItunesTrack {
    #[serde(rename = "trackId")]
    pub track_id: u32,
    #[serde(rename = "trackName")]
    pub track_name: String,
    #[serde(rename = "artistName")]
    pub artist_name: String,
    #[serde(rename = "collectionName")]
    pub collection_name: String,
    #[serde(rename = "artworkUrl100")]
    pub artwork_url_100: String,
    #[serde(rename = "previewUrl")]
    pub preview_url: String,
    #[serde(rename = "trackTimeMillis")]
    pub track_time_millis: Option<u32>,
    #[serde(rename = "primaryGenreName")]
    pub primary_genre_name: String,
}

pub fn search(query: &str) -> Result<Vec<ItunesTrack>, String> {
    let encoded = url_encode(query);
    let url = format!(
        "https://itunes.apple.com/search?term={}&media=music&entity=song&limit=25",
        encoded
    );

    let mut resp = ureq::get(&url).call().map_err(|e| e.to_string())?;
    let body: String = resp
        .body_mut()
        .read_to_string()
        .map_err(|e| e.to_string())?;

    let data: ItunesResponse = serde_json::from_str(&body).map_err(|e| e.to_string())?;
    Ok(data.results)
}

fn url_encode(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
            ' ' => "+".to_string(),
            _ => format!("%{:02X}", c as u8),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_itunes_response() {
        let json = r#"{
            "resultCount": 2,
            "results": [
                {
                    "wrapperType": "track",
                    "kind": "song",
                    "trackId": 12345,
                    "artistName": "Eminem",
                    "trackName": "Lose Yourself",
                    "collectionName": "8 Mile Soundtrack",
                    "artworkUrl100": "https://example.com/artwork1.jpg",
                    "previewUrl": "https://example.com/preview1.m4a",
                    "trackTimeMillis": 312000,
                    "primaryGenreName": "Hip-Hop/Rap",
                    "trackNumber": 1,
                    "discNumber": 1
                },
                {
                    "wrapperType": "track",
                    "kind": "song",
                    "trackId": 67890,
                    "artistName": "Daft Punk",
                    "trackName": "One More Time",
                    "collectionName": "Discovery",
                    "artworkUrl100": "https://example.com/artwork2.jpg",
                    "previewUrl": "https://example.com/preview2.m4a",
                    "trackTimeMillis": null,
                    "primaryGenreName": "Electronic",
                    "trackNumber": 2,
                    "discNumber": 1
                }
            ]
        }"#;

        let response: ItunesResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.result_count, 2);
        assert_eq!(response.results.len(), 2);

        let t1 = &response.results[0];
        assert_eq!(t1.track_id, 12345);
        assert_eq!(t1.track_name, "Lose Yourself");
        assert_eq!(t1.artist_name, "Eminem");
        assert_eq!(t1.collection_name, "8 Mile Soundtrack");
        assert_eq!(t1.preview_url, "https://example.com/preview1.m4a");
        assert_eq!(t1.track_time_millis, Some(312000));
        assert_eq!(t1.primary_genre_name, "Hip-Hop/Rap");

        let t2 = &response.results[1];
        assert_eq!(t2.track_id, 67890);
        assert_eq!(t2.track_name, "One More Time");
        assert_eq!(t2.track_time_millis, None);
    }

    #[test]
    fn parse_empty_results() {
        let json = r#"{"resultCount":0,"results":[]}"#;
        let response: ItunesResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.result_count, 0);
        assert!(response.results.is_empty());
    }

    #[test]
    fn url_encode_plain_text() {
        let encoded = url_encode("hello world");
        assert_eq!(encoded, "hello+world");
    }

    #[test]
    fn url_encode_special_chars() {
        let encoded = url_encode("artist & song");
        assert_eq!(encoded, "artist+%26+song");
    }
}
