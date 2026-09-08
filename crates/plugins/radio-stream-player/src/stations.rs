use rusqlite::Connection;
use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize)]
pub struct Station {
    pub name: String,
    pub url: String,
    pub country: String,
    pub genre: String,
}

impl Station {
    pub fn country_name(&self) -> &str {
        country_name(&self.country)
    }
}

pub fn load(conn: &Connection) -> Vec<Station> {
    crate::database::load_all(conn).unwrap_or_else(|e| {
        log::warn!("  ⚠️  SQLite load_all failed ({e})");
        Vec::new()
    })
}

/// Server-first catalog load with local SQLite fallback.
///
/// Tries `{SANTUI_API_URL}/api/v1/stations` (default `https://api3.sony-ak.com`)
/// so station data can be maintained centrally; any failure — network error,
/// non-200 status, invalid JSON, or an empty catalog — falls back to the local
/// database, which always works offline. Kept separate from [`load`] so the
/// local path stays directly testable without network access.
pub fn load_remote_or_local(conn: &Connection) -> Vec<Station> {
    match fetch_remote() {
        Ok(remote) if !remote.is_empty() => {
            log::info!(
                "  📡 stations loaded from {} ({} stations)",
                api_base_url(),
                remote.len()
            );
            remote
        }
        Ok(_) => {
            log::warn!("  ⚠️  remote catalog empty, falling back to local SQLite");
            load(conn)
        }
        Err(e) => {
            log::warn!("  ⚠️  remote stations failed ({e}), falling back to local SQLite");
            load(conn)
        }
    }
}

pub const DEFAULT_API_BASE_URL: &str = "https://api3.sony-ak.com";
const REMOTE_PAGE_LIMIT: i64 = 20_000;

/// Pure base-URL resolution (trailing slashes trimmed); reads no environment
/// so it stays hermetic under test. See [`api_base_url`].
pub fn api_base_url_from(env_value: Option<&str>) -> String {
    let base = env_value
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(DEFAULT_API_BASE_URL);
    base.trim_end_matches('/').to_string()
}

pub fn api_base_url() -> String {
    api_base_url_from(std::env::var("SANTUI_API_URL").ok().as_deref())
}

#[derive(Deserialize)]
struct StationsPage {
    #[serde(default)]
    stations: Vec<Station>,
    #[serde(default)]
    total: i64,
}

fn parse_stations_response(text: &str) -> Result<StationsPage, String> {
    serde_json::from_str(text).map_err(|e| format!("stations parse failed: {e}"))
}

fn fetch_page(base: &str, limit: i64, offset: i64) -> Result<StationsPage, String> {
    let url = format!("{base}/api/v1/stations?limit={limit}&offset={offset}");
    let mut resp = crate::http::agent()
        .get(&url)
        .call()
        .map_err(|e| format!("stations request failed: {e}"))?;
    let body = resp
        .body_mut()
        .read_to_string()
        .map_err(|e| format!("stations read body failed: {e}"))?;
    parse_stations_response(&body)
}

/// Fetch the full remote catalog, following pages if it outgrows one page.
pub fn fetch_remote() -> Result<Vec<Station>, String> {
    let base = api_base_url();
    let mut all = Vec::new();
    let mut offset = 0i64;
    loop {
        let page = fetch_page(&base, REMOTE_PAGE_LIMIT, offset)?;
        if page.stations.is_empty() {
            break;
        }
        offset += page.stations.len() as i64;
        let done = page.total <= 0 || offset >= page.total;
        all.extend(page.stations);
        if done {
            break;
        }
    }
    Ok(all)
}

pub fn reload(conn: &Connection) -> Vec<Station> {
    crate::database::load_all(conn).unwrap_or_default()
}

pub fn country_name(code: &str) -> &str {
    COUNTRY_NAMES
        .binary_search_by_key(&code, |(k, _)| k)
        .map(|i| COUNTRY_NAMES[i].1)
        .unwrap_or(code)
}

const COUNTRY_NAMES: &[(&str, &str)] = &[
    ("AD", "Andorra"),
    ("AE", "United Arab Emirates"),
    ("AF", "Afghanistan"),
    ("AG", "Antigua and Barbuda"),
    ("AI", "Anguilla"),
    ("AL", "Albania"),
    ("AM", "Armenia"),
    ("AO", "Angola"),
    ("AR", "Argentina"),
    ("AS", "American Samoa"),
    ("AT", "Austria"),
    ("AU", "Australia"),
    ("AW", "Aruba"),
    ("AZ", "Azerbaijan"),
    ("BA", "Bosnia and Herzegovina"),
    ("BB", "Barbados"),
    ("BD", "Bangladesh"),
    ("BE", "Belgium"),
    ("BF", "Burkina Faso"),
    ("BG", "Bulgaria"),
    ("BH", "Bahrain"),
    ("BI", "Burundi"),
    ("BJ", "Benin"),
    ("BL", "Saint Barthélemy"),
    ("BM", "Bermuda"),
    ("BN", "Brunei"),
    ("BO", "Bolivia"),
    ("BQ", "Bonaire"),
    ("BR", "Brazil"),
    ("BS", "Bahamas"),
    ("BT", "Bhutan"),
    ("BW", "Botswana"),
    ("BZ", "Belize"),
    ("CA", "Canada"),
    ("CD", "Congo DR"),
    ("CF", "Central African Republic"),
    ("CG", "Congo"),
    ("CH", "Switzerland"),
    ("CI", "Côte d'Ivoire"),
    ("CK", "Cook Islands"),
    ("CL", "Chile"),
    ("CM", "Cameroon"),
    ("CN", "China"),
    ("CO", "Colombia"),
    ("CR", "Costa Rica"),
    ("CU", "Cuba"),
    ("CV", "Cape Verde"),
    ("CW", "Curaçao"),
    ("CY", "Cyprus"),
    ("CZ", "Czech Republic"),
    ("DE", "Germany"),
    ("DJ", "Djibouti"),
    ("DK", "Denmark"),
    ("DM", "Dominica"),
    ("DO", "Dominican Republic"),
    ("DZ", "Algeria"),
    ("EC", "Ecuador"),
    ("EE", "Estonia"),
    ("EG", "Egypt"),
    ("EH", "Western Sahara"),
    ("ER", "Eritrea"),
    ("ES", "Spain"),
    ("ET", "Ethiopia"),
    ("FI", "Finland"),
    ("FJ", "Fiji"),
    ("FK", "Falkland Islands"),
    ("FM", "Micronesia"),
    ("FO", "Faroe Islands"),
    ("FR", "France"),
    ("GA", "Gabon"),
    ("GB", "United Kingdom"),
    ("GD", "Grenada"),
    ("GE", "Georgia"),
    ("GF", "French Guiana"),
    ("GG", "Guernsey"),
    ("GH", "Ghana"),
    ("GI", "Gibraltar"),
    ("GL", "Greenland"),
    ("GM", "Gambia"),
    ("GN", "Guinea"),
    ("GP", "Guadeloupe"),
    ("GQ", "Equatorial Guinea"),
    ("GR", "Greece"),
    ("GT", "Guatemala"),
    ("GU", "Guam"),
    ("GW", "Guinea-Bissau"),
    ("GY", "Guyana"),
    ("HK", "Hong Kong"),
    ("HN", "Honduras"),
    ("HR", "Croatia"),
    ("HT", "Haiti"),
    ("HU", "Hungary"),
    ("ID", "Indonesia"),
    ("IE", "Ireland"),
    ("IL", "Israel"),
    ("IM", "Isle of Man"),
    ("IN", "India"),
    ("IQ", "Iraq"),
    ("IR", "Iran"),
    ("IS", "Iceland"),
    ("IT", "Italy"),
    ("JE", "Jersey"),
    ("JM", "Jamaica"),
    ("JO", "Jordan"),
    ("JP", "Japan"),
    ("KE", "Kenya"),
    ("KG", "Kyrgyzstan"),
    ("KH", "Cambodia"),
    ("KI", "Kiribati"),
    ("KM", "Comoros"),
    ("KN", "Saint Kitts and Nevis"),
    ("KR", "South Korea"),
    ("KW", "Kuwait"),
    ("KY", "Cayman Islands"),
    ("KZ", "Kazakhstan"),
    ("LA", "Laos"),
    ("LB", "Lebanon"),
    ("LC", "Saint Lucia"),
    ("LI", "Liechtenstein"),
    ("LK", "Sri Lanka"),
    ("LR", "Liberia"),
    ("LS", "Lesotho"),
    ("LT", "Lithuania"),
    ("LU", "Luxembourg"),
    ("LV", "Latvia"),
    ("LY", "Libya"),
    ("MA", "Morocco"),
    ("MC", "Monaco"),
    ("MD", "Moldova"),
    ("ME", "Montenegro"),
    ("MF", "Saint Martin"),
    ("MG", "Madagascar"),
    ("MH", "Marshall Islands"),
    ("MK", "North Macedonia"),
    ("ML", "Mali"),
    ("MM", "Myanmar"),
    ("MN", "Mongolia"),
    ("MO", "Macau"),
    ("MP", "Northern Mariana Islands"),
    ("MQ", "Martinique"),
    ("MR", "Mauritania"),
    ("MS", "Montserrat"),
    ("MT", "Malta"),
    ("MU", "Mauritius"),
    ("MV", "Maldives"),
    ("MW", "Malawi"),
    ("MX", "Mexico"),
    ("MY", "Malaysia"),
    ("MZ", "Mozambique"),
    ("NA", "Namibia"),
    ("NC", "New Caledonia"),
    ("NE", "Niger"),
    ("NF", "Norfolk Island"),
    ("NG", "Nigeria"),
    ("NI", "Nicaragua"),
    ("NL", "Netherlands"),
    ("NO", "Norway"),
    ("NP", "Nepal"),
    ("NR", "Nauru"),
    ("NU", "Niue"),
    ("NZ", "New Zealand"),
    ("OM", "Oman"),
    ("PA", "Panama"),
    ("PE", "Peru"),
    ("PF", "French Polynesia"),
    ("PG", "Papua New Guinea"),
    ("PH", "Philippines"),
    ("PK", "Pakistan"),
    ("PL", "Poland"),
    ("PM", "Saint Pierre and Miquelon"),
    ("PR", "Puerto Rico"),
    ("PS", "Palestine"),
    ("PT", "Portugal"),
    ("PW", "Palau"),
    ("PY", "Paraguay"),
    ("QA", "Qatar"),
    ("RE", "Réunion"),
    ("RO", "Romania"),
    ("RS", "Serbia"),
    ("RU", "Russia"),
    ("RW", "Rwanda"),
    ("SA", "Saudi Arabia"),
    ("SB", "Solomon Islands"),
    ("SC", "Seychelles"),
    ("SD", "Sudan"),
    ("SE", "Sweden"),
    ("SG", "Singapore"),
    ("SH", "Saint Helena"),
    ("SI", "Slovenia"),
    ("SJ", "Svalbard and Jan Mayen"),
    ("SK", "Slovakia"),
    ("SL", "Sierra Leone"),
    ("SM", "San Marino"),
    ("SN", "Senegal"),
    ("SO", "Somalia"),
    ("SR", "Suriname"),
    ("SS", "South Sudan"),
    ("ST", "São Tomé and Príncipe"),
    ("SV", "El Salvador"),
    ("SX", "Sint Maarten"),
    ("SY", "Syria"),
    ("SZ", "Eswatini"),
    ("TC", "Turks and Caicos Islands"),
    ("TD", "Chad"),
    ("TG", "Togo"),
    ("TH", "Thailand"),
    ("TJ", "Tajikistan"),
    ("TK", "Tokelau"),
    ("TL", "Timor-Leste"),
    ("TM", "Turkmenistan"),
    ("TN", "Tunisia"),
    ("TO", "Tonga"),
    ("TR", "Turkey"),
    ("TT", "Trinidad and Tobago"),
    ("TV", "Tuvalu"),
    ("TW", "Taiwan"),
    ("TZ", "Tanzania"),
    ("UA", "Ukraine"),
    ("UG", "Uganda"),
    ("US", "United States"),
    ("UY", "Uruguay"),
    ("UZ", "Uzbekistan"),
    ("VA", "Vatican City"),
    ("VC", "Saint Vincent and the Grenadines"),
    ("VE", "Venezuela"),
    ("VG", "British Virgin Islands"),
    ("VI", "U.S. Virgin Islands"),
    ("VN", "Vietnam"),
    ("VU", "Vanuatu"),
    ("WF", "Wallis and Futuna"),
    ("WS", "Samoa"),
    ("XK", "Kosovo"),
    ("YE", "Yemen"),
    ("YT", "Mayotte"),
    ("ZA", "South Africa"),
    ("ZM", "Zambia"),
    ("ZW", "Zimbabwe"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn country_name_first_entry() {
        assert_eq!(country_name("AD"), "Andorra");
    }

    #[test]
    fn country_name_last_entry() {
        assert_eq!(country_name("ZW"), "Zimbabwe");
    }

    #[test]
    fn country_name_middle_entries() {
        assert_eq!(country_name("US"), "United States");
        assert_eq!(country_name("GB"), "United Kingdom");
        assert_eq!(country_name("JP"), "Japan");
        assert_eq!(country_name("BR"), "Brazil");
    }

    #[test]
    fn country_name_unknown_code_returns_code() {
        assert_eq!(country_name("XX"), "XX");
    }

    #[test]
    fn country_name_empty_returns_empty() {
        assert_eq!(country_name(""), "");
    }

    #[test]
    fn country_name_case_sensitive() {
        assert_eq!(country_name("us"), "us");
        assert_eq!(country_name("Us"), "Us");
    }

    #[test]
    fn country_name_near_boundary() {
        assert_eq!(country_name("AF"), "Afghanistan");
        assert_eq!(country_name("AG"), "Antigua and Barbuda");
    }

    fn setup_db() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE stations (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                url TEXT NOT NULL,
                country TEXT NOT NULL DEFAULT '',
                genre TEXT NOT NULL DEFAULT ''
            )",
        )
        .unwrap();
        conn
    }

    #[test]
    fn load_returns_all_stations() {
        let conn = setup_db();
        conn.execute(
            "INSERT INTO stations (name, url, country) VALUES (?1, ?2, ?3)",
            rusqlite::params!["Station A", "http://a", "US"],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO stations (name, url, country) VALUES (?1, ?2, ?3)",
            rusqlite::params!["Station B", "http://b", "GB"],
        )
        .unwrap();
        let stations = load(&conn);
        assert_eq!(stations.len(), 2);
        assert_eq!(stations[0].name, "Station A");
        assert_eq!(stations[1].name, "Station B");
    }

    #[test]
    fn load_empty_db_returns_empty_vec() {
        let conn = setup_db();
        let stations = load(&conn);
        assert!(stations.is_empty());
    }

    #[test]
    fn load_orders_by_name() {
        let conn = setup_db();
        conn.execute(
            "INSERT INTO stations (name, url) VALUES (?1, ?2)",
            rusqlite::params!["Zeta", "http://z"],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO stations (name, url) VALUES (?1, ?2)",
            rusqlite::params!["Alpha", "http://a"],
        )
        .unwrap();
        let stations = load(&conn);
        assert_eq!(stations[0].name, "Alpha");
        assert_eq!(stations[1].name, "Zeta");
    }

    #[test]
    fn reload_returns_all_stations() {
        let conn = setup_db();
        conn.execute(
            "INSERT INTO stations (name, url) VALUES (?1, ?2)",
            rusqlite::params!["Test", "http://test"],
        )
        .unwrap();
        let stations = reload(&conn);
        assert_eq!(stations.len(), 1);
        assert_eq!(stations[0].name, "Test");
    }

    #[test]
    fn reload_empty_db_returns_empty() {
        let conn = setup_db();
        let stations = reload(&conn);
        assert!(stations.is_empty());
    }

    #[test]
    fn station_country_name_delegates() {
        let s = Station {
            name: "X".into(),
            url: "http://x".into(),
            country: "US".into(),
            genre: String::new(),
        };
        assert_eq!(s.country_name(), "United States");
    }

    #[test]
    fn station_country_name_unknown_code() {
        let s = Station {
            name: "X".into(),
            url: "http://x".into(),
            country: "XX".into(),
            genre: String::new(),
        };
        assert_eq!(s.country_name(), "XX");
    }

    #[test]
    fn api_base_url_defaults_and_trims() {
        assert_eq!(api_base_url_from(None), DEFAULT_API_BASE_URL);
        assert_eq!(api_base_url_from(Some("")), DEFAULT_API_BASE_URL);
        assert_eq!(api_base_url_from(Some("   ")), DEFAULT_API_BASE_URL);
        assert_eq!(
            api_base_url_from(Some("http://localhost:9876")),
            "http://localhost:9876"
        );
        assert_eq!(
            api_base_url_from(Some("https://api3.sony-ak.com///")),
            "https://api3.sony-ak.com"
        );
    }

    #[test]
    fn parse_stations_response_valid() {
        let body = r#"{"stations":[{"name":"Rock FM","url":"http://rock","country":"US","genre":"Rock"}],"total":1,"limit":20000,"offset":0}"#;
        let page = parse_stations_response(body).unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.stations.len(), 1);
        assert_eq!(page.stations[0].name, "Rock FM");
        assert_eq!(page.stations[0].genre, "Rock");
    }

    #[test]
    fn parse_stations_response_tolerates_shape_drift() {
        // Extra fields ignored; missing optional fields defaulted.
        let body = r#"{"stations":[],"total":0,"limit":1,"offset":0,"extra":{}}"#;
        let page = parse_stations_response(body).unwrap();
        assert!(page.stations.is_empty());
        let body = r#"{"stations":[]}"#;
        let page = parse_stations_response(body).unwrap();
        assert_eq!(page.total, 0);
    }

    #[test]
    fn parse_stations_response_rejects_garbage() {
        assert!(parse_stations_response("not json").is_err());
        assert!(parse_stations_response(r#"{"stations":"nope"}"#).is_err());
        // Cloudflare-style HTML error page must not parse as stations.
        assert!(parse_stations_response("<html><body>error</body></html>").is_err());
    }

    const STUB_PAGE_1: &str = r#"{"stations":[{"name":"A","url":"http://a","country":"US","genre":"Rock"},{"name":"B","url":"http://b","country":"GB","genre":"Pop"}],"total":3,"limit":20000,"offset":0}"#;
    const STUB_PAGE_2: &str = r#"{"stations":[{"name":"C","url":"http://c","country":"DE","genre":"Jazz"}],"total":3,"limit":20000,"offset":2}"#;

    #[test]
    fn fetch_remote_paginates_against_stub_server() {
        use std::io::{Read, Write};

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = std::thread::spawn(move || {
            for stream in listener.incoming().take(2) {
                let mut s = stream.unwrap();
                let mut buf = [0u8; 4096];
                let n = s.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]).into_owned();
                let body = if req.contains("offset=2") {
                    STUB_PAGE_2
                } else {
                    STUB_PAGE_1
                };
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                s.write_all(resp.as_bytes()).unwrap();
            }
        });

        // SAFETY: no other test in this binary reads or writes SANTUI_API_URL,
        // so scoped mutation here cannot race.
        unsafe {
            std::env::set_var("SANTUI_API_URL", format!("http://{addr}"));
        }
        let result = fetch_remote();
        unsafe {
            std::env::remove_var("SANTUI_API_URL");
        }
        handle.join().unwrap();

        let stations = result.unwrap();
        assert_eq!(stations.len(), 3);
        assert_eq!(stations[2].name, "C");
    }
}
