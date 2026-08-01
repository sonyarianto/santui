use serde::{Deserialize, Serialize};

use santui_ipc::text::url_encode;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItunesResponse {
    #[serde(rename = "resultCount")]
    pub result_count: u32,
    pub results: Vec<ItunesTrack>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItunesTrack {
    #[serde(rename = "trackId")]
    pub track_id: u64,
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
    #[serde(rename = "releaseDate", default)]
    pub release_date: String,
    #[serde(rename = "trackNumber", default)]
    pub track_number: Option<u32>,
    #[serde(rename = "discNumber", default)]
    pub disc_number: Option<u32>,
    #[serde(rename = "country", default)]
    pub country: String,
    #[serde(rename = "currency", default)]
    pub currency: String,
    #[serde(rename = "trackPrice", default)]
    pub track_price: Option<f64>,
    #[serde(rename = "collectionPrice", default)]
    pub collection_price: Option<f64>,
    #[serde(rename = "trackExplicitness", default)]
    pub track_explicitness: String,
}

pub fn search(query: &str) -> Result<Vec<ItunesTrack>, String> {
    let encoded = url_encode(query);
    let url = format!(
        "https://itunes.apple.com/search?term={}&media=music&entity=song&limit=200",
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
                    "releaseDate": "2002-10-29T08:00:00Z",
                    "trackNumber": 1,
                    "discNumber": 1,
                    "country": "USA",
                    "currency": "USD",
                    "trackPrice": 1.29,
                    "collectionPrice": 9.99,
                    "trackExplicitness": "notExplicit"
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
        assert_eq!(t1.release_date, "2002-10-29T08:00:00Z");
        assert_eq!(t1.track_number, Some(1));
        assert_eq!(t1.disc_number, Some(1));
        assert_eq!(t1.country, "USA");
        assert_eq!(t1.currency, "USD");
        assert_eq!(t1.track_price, Some(1.29));
        assert_eq!(t1.collection_price, Some(9.99));
        assert_eq!(t1.track_explicitness, "notExplicit");

        let t2 = &response.results[1];
        assert_eq!(t2.track_id, 67890);
        assert_eq!(t2.track_name, "One More Time");
        assert_eq!(t2.track_time_millis, None);
        assert_eq!(t2.track_number, Some(2));
        assert_eq!(t2.release_date, "");
        assert_eq!(t2.track_price, None);
    }

    #[test]
    fn parse_empty_results() {
        let json = r#"{"resultCount":0,"results":[]}"#;
        let response: ItunesResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.result_count, 0);
        assert!(response.results.is_empty());
    }
}
