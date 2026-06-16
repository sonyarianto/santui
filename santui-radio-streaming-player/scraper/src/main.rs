//! Scrapes internet radio stations from an online directory
//! and saves them to the Santui radio streaming player's SQLite database.
//!
//! ## Usage
//!
//! ```bash
//! cargo run -p santui-radio-streaming-scraper
//! ```
//!
//! Fetches the currently-playing station list from every country via the
//! `?nowlisten=1` endpoint. Deduplicates by `(name, url)` using `UNIQUE`
//! constraint + `INSERT OR IGNORE`. Run periodically to keep stations fresh.
//!
//! The database is shared with the radio plugin at:
//! - **Windows**: `%APPDATA%\santui\radio_streaming_stations.db`
//! - **Linux/macOS**: `~/.local/share/santui/radio_streaming_stations.db`
//!
//! Press `r` in the radio player plugin to reload stations from the DB.

use rusqlite::Connection;
use std::path::PathBuf;

/// All ISO 3166-1 alpha-2 country codes that onlineradiobox.com uses.
/// Source: https://onlineradiobox.com uses these codes in URLs like /{code}/
const ALL_COUNTRIES: &[(&str, &str)] = &[
    ("af", "AF"),
    ("al", "AL"),
    ("dz", "DZ"),
    ("as", "AS"),
    ("ad", "AD"),
    ("ao", "AO"),
    ("ai", "AI"),
    ("ag", "AG"),
    ("ar", "AR"),
    ("am", "AM"),
    ("aw", "AW"),
    ("au", "AU"),
    ("at", "AT"),
    ("az", "AZ"),
    ("bs", "BS"),
    ("bh", "BH"),
    ("bd", "BD"),
    ("bb", "BB"),
    ("be", "BE"),
    ("bz", "BZ"),
    ("bj", "BJ"),
    ("bm", "BM"),
    ("bt", "BT"),
    ("bo", "BO"),
    ("bq", "BQ"),
    ("ba", "BA"),
    ("bw", "BW"),
    ("br", "BR"),
    ("bn", "BN"),
    ("bg", "BG"),
    ("bf", "BF"),
    ("bi", "BI"),
    ("cv", "CV"),
    ("kh", "KH"),
    ("cm", "CM"),
    ("ca", "CA"),
    ("ky", "KY"),
    ("cf", "CF"),
    ("td", "TD"),
    ("cl", "CL"),
    ("cn", "CN"),
    ("co", "CO"),
    ("km", "KM"),
    ("cg", "CG"),
    ("cd", "CD"),
    ("ck", "CK"),
    ("cr", "CR"),
    ("ci", "CI"),
    ("hr", "HR"),
    ("cu", "CU"),
    ("cw", "CW"),
    ("cy", "CY"),
    ("cz", "CZ"),
    ("dk", "DK"),
    ("dj", "DJ"),
    ("dm", "DM"),
    ("do", "DO"),
    ("ec", "EC"),
    ("eg", "EG"),
    ("sv", "SV"),
    ("gq", "GQ"),
    ("er", "ER"),
    ("ee", "EE"),
    ("sz", "SZ"),
    ("et", "ET"),
    ("fk", "FK"),
    ("fo", "FO"),
    ("fj", "FJ"),
    ("fi", "FI"),
    ("fr", "FR"),
    ("gf", "GF"),
    ("pf", "PF"),
    ("ga", "GA"),
    ("gm", "GM"),
    ("ge", "GE"),
    ("de", "DE"),
    ("gh", "GH"),
    ("gi", "GI"),
    ("gr", "GR"),
    ("gl", "GL"),
    ("gd", "GD"),
    ("gp", "GP"),
    ("gu", "GU"),
    ("gt", "GT"),
    ("gg", "GG"),
    ("gn", "GN"),
    ("gw", "GW"),
    ("gy", "GY"),
    ("ht", "HT"),
    ("hn", "HN"),
    ("hk", "HK"),
    ("hu", "HU"),
    ("is", "IS"),
    ("in", "IN"),
    ("id", "ID"),
    ("ir", "IR"),
    ("iq", "IQ"),
    ("ie", "IE"),
    ("im", "IM"),
    ("il", "IL"),
    ("it", "IT"),
    ("jm", "JM"),
    ("jp", "JP"),
    ("je", "JE"),
    ("jo", "JO"),
    ("kz", "KZ"),
    ("ke", "KE"),
    ("ki", "KI"),
    ("kr", "KR"),
    ("xk", "XK"),
    ("kw", "KW"),
    ("kg", "KG"),
    ("la", "LA"),
    ("lv", "LV"),
    ("lb", "LB"),
    ("ls", "LS"),
    ("lr", "LR"),
    ("ly", "LY"),
    ("li", "LI"),
    ("lt", "LT"),
    ("lu", "LU"),
    ("mg", "MG"),
    ("mw", "MW"),
    ("my", "MY"),
    ("mv", "MV"),
    ("ml", "ML"),
    ("mt", "MT"),
    ("mh", "MH"),
    ("mq", "MQ"),
    ("mr", "MR"),
    ("mu", "MU"),
    ("yt", "YT"),
    ("mx", "MX"),
    ("fm", "FM"),
    ("md", "MD"),
    ("mc", "MC"),
    ("mn", "MN"),
    ("me", "ME"),
    ("ms", "MS"),
    ("ma", "MA"),
    ("mz", "MZ"),
    ("mm", "MM"),
    ("na", "NA"),
    ("nr", "NR"),
    ("np", "NP"),
    ("nl", "NL"),
    ("nc", "NC"),
    ("nz", "NZ"),
    ("ni", "NI"),
    ("ne", "NE"),
    ("ng", "NG"),
    ("mk", "MK"),
    ("mp", "MP"),
    ("no", "NO"),
    ("om", "OM"),
    ("pk", "PK"),
    ("pw", "PW"),
    ("ps", "PS"),
    ("pa", "PA"),
    ("pg", "PG"),
    ("py", "PY"),
    ("pe", "PE"),
    ("ph", "PH"),
    ("pl", "PL"),
    ("pt", "PT"),
    ("pr", "PR"),
    ("qa", "QA"),
    ("re", "RE"),
    ("ro", "RO"),
    ("rw", "RW"),
    ("bl", "BL"),
    ("kn", "KN"),
    ("lc", "LC"),
    ("mf", "MF"),
    ("pm", "PM"),
    ("vc", "VC"),
    ("ws", "WS"),
    ("sm", "SM"),
    ("st", "ST"),
    ("sa", "SA"),
    ("sn", "SN"),
    ("rs", "RS"),
    ("sc", "SC"),
    ("sl", "SL"),
    ("sg", "SG"),
    ("sx", "SX"),
    ("sk", "SK"),
    ("si", "SI"),
    ("sb", "SB"),
    ("so", "SO"),
    ("za", "ZA"),
    ("ss", "SS"),
    ("es", "ES"),
    ("lk", "LK"),
    ("sd", "SD"),
    ("sr", "SR"),
    ("sj", "SJ"),
    ("se", "SE"),
    ("ch", "CH"),
    ("sy", "SY"),
    ("tw", "TW"),
    ("tj", "TJ"),
    ("tz", "TZ"),
    ("th", "TH"),
    ("tl", "TL"),
    ("tg", "TG"),
    ("tk", "TK"),
    ("to", "TO"),
    ("tt", "TT"),
    ("tn", "TN"),
    ("tr", "TR"),
    ("tm", "TM"),
    ("tc", "TC"),
    ("tv", "TV"),
    ("ug", "UG"),
    ("ae", "AE"),
    ("us", "US"),
    ("uy", "UY"),
    ("uz", "UZ"),
    ("vu", "VU"),
    ("va", "VA"),
    ("ve", "VE"),
    ("vn", "VN"),
    ("vg", "VG"),
    ("vi", "VI"),
    ("wf", "WF"),
    ("eh", "EH"),
    ("ye", "YE"),
    ("zm", "ZM"),
    ("zw", "ZW"),
    // Non-standard codes used by onlineradiobox
    ("uk", "GB"), // onlineradiobox uses 'uk' for United Kingdom
];

#[derive(serde::Deserialize)]
struct RadioBoxResponse {
    data: String,
}

fn app_data_dir() -> PathBuf {
    let path = if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local").join("share"))
            })
    };
    path.unwrap_or_else(|| PathBuf::from(".")).join("santui")
}

fn db_path() -> PathBuf {
    app_data_dir().join("radio_streaming_stations.db")
}

fn open_db() -> Result<Connection, rusqlite::Error> {
    let path = db_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let conn = Connection::open(&path)?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS stations (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            url TEXT NOT NULL,
            country TEXT NOT NULL DEFAULT ''
        );
        CREATE UNIQUE INDEX IF NOT EXISTS idx_stations_name_url ON stations(name, url);
        CREATE INDEX IF NOT EXISTS idx_stations_country ON stations(country);",
    )?;
    Ok(conn)
}

fn extract_stations(html: &str) -> Vec<(String, String)> {
    let mut stations = Vec::new();
    let search = r#"class="b-play station_play"#;
    let mut pos = 0;
    while let Some(start) = html[pos..].find(search) {
        let fragment_start = pos + start;
        let fragment = &html[fragment_start..];

        let stream = extract_attr(fragment, "stream");
        let name = extract_attr(fragment, "radioName");

        if let (Some(url), Some(name)) = (stream, name) {
            let name = unescape_html(&name);
            if !url.is_empty() && !name.is_empty() {
                let url = url
                    .replace("?dist=onlineradiobox", "")
                    .replace("&dist=onlineradiobox", "")
                    .replace("?ref=onlineradiobox26", "")
                    .replace("&ref=onlineradiobox26", "");
                stations.push((name, url));
            }
        }

        pos = fragment_start + 1;
    }
    stations
}

fn extract_attr(fragment: &str, attr: &str) -> Option<String> {
    let search = format!(" {attr}=\"");
    let start = fragment.find(&search)?;
    let value_start = start + search.len();
    let end = fragment[value_start..].find('"')?;
    Some(fragment[value_start..value_start + end].to_string())
}

fn unescape_html(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&#39;", "'")
        .replace("&quot;", "\"")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

fn fetch_country_http(url_code: &str) -> Result<Vec<(String, String)>, String> {
    let url = format!("https://onlineradiobox.com/{url_code}/?nowlisten=1");
    let body = ureq::get(&url)
        .set("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .set("Accept", "application/json, text/plain, */*")
        .call()
        .map_err(|e| format!("request failed: {e}"))?
        .into_string()
        .map_err(|e| format!("failed to read response: {e}"))?;

    let resp: RadioBoxResponse =
        serde_json::from_str(&body).map_err(|e| format!("JSON parse error: {e}"))?;

    Ok(extract_stations(&resp.data))
}

fn main() {
    println!("Radio Station Scraper");
    let num_workers: usize = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    println!(
        "{} countries to scan ({} workers)",
        ALL_COUNTRIES.len(),
        num_workers
    );
    println!();

    let conn = match open_db() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to open database: {e}");
            std::process::exit(1);
        }
    };

    // Clean up ?dist=onlineradiobox and ?ref=onlineradiobox26 from existing URLs
    // First, remove duplicates where the cleaned URL already exists
    let deleted = conn
        .execute(
            "DELETE FROM stations WHERE (url LIKE '%dist=onlineradiobox%' OR url LIKE '%ref=onlineradiobox26%') AND EXISTS (
                SELECT 1 FROM stations AS s2
                WHERE s2.name = stations.name
                AND s2.url = REPLACE(REPLACE(REPLACE(REPLACE(stations.url, '?dist=onlineradiobox', ''), '&dist=onlineradiobox', ''), '?ref=onlineradiobox26', ''), '&ref=onlineradiobox26', '')
            )",
            [],
        )
        .unwrap_or(0);
    if deleted > 0 {
        println!("Removed {deleted} duplicate stations with tracking params");
    }
    // Then clean the remaining
    let cleaned = conn
        .execute(
            "UPDATE stations SET url = REPLACE(REPLACE(REPLACE(REPLACE(url, '?dist=onlineradiobox', ''), '&dist=onlineradiobox', ''), '?ref=onlineradiobox26', ''), '&ref=onlineradiobox26', '') WHERE url LIKE '%dist=onlineradiobox%' OR url LIKE '%ref=onlineradiobox26%'",
            [],
        )
        .unwrap_or(0);
    if cleaned > 0 {
        println!("Cleaned {cleaned} existing URLs");
    }

    conn.execute_batch("BEGIN TRANSACTION")
        .expect("begin transaction");

    let (tx, rx) = std::sync::mpsc::channel::<(String, String, Vec<(String, String)>)>();
    let countries: Vec<(&str, &str)> = ALL_COUNTRIES.to_vec();
    let chunk_size = countries.len().div_ceil(num_workers);

    let mut total_fetched = 0usize;
    let mut countries_with_data = 0usize;

    std::thread::scope(|s| {
        for chunk in countries.chunks(chunk_size) {
            let tx = tx.clone();
            let chunk: Vec<(&str, &str)> = chunk.to_vec();
            s.spawn(move || {
                for &(url_code, iso_code) in &chunk {
                    match fetch_country_http(url_code) {
                        Ok(stations) => {
                            let _ = tx.send((iso_code.to_string(), url_code.to_string(), stations));
                        }
                        Err(e) => {
                            eprintln!("  \u{26a0}\u{fe0f}  {url_code}: {e}");
                        }
                    }
                }
            });
        }
        drop(tx);

        for (iso_code, _url_code, stations) in rx {
            if stations.is_empty() {
                continue;
            }
            countries_with_data += 1;
            total_fetched += stations.len();

            let mut inserted = 0usize;
            for (name, url) in &stations {
                match conn.execute(
                    "INSERT OR IGNORE INTO stations (name, url, country) VALUES (?1, ?2, ?3)",
                    rusqlite::params![name, url, iso_code],
                ) {
                    Ok(rows) => {
                        if rows > 0 {
                            inserted += 1;
                        }
                    }
                    Err(e) => {
                        eprintln!("  \u{26a0}\u{fe0f}  insert error for {name}: {e}");
                    }
                }
            }

            let total: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM stations WHERE country = ?1",
                    rusqlite::params![iso_code],
                    |row| row.get(0),
                )
                .unwrap_or(0);

            println!(
                "  {iso_code}: +{inserted} new (={total} total, {} fetched)",
                stations.len()
            );
        }
    });

    conn.execute_batch("COMMIT").expect("commit transaction");

    let total: i64 = conn
        .query_row("SELECT COUNT(*) FROM stations", [], |row| row.get(0))
        .unwrap_or(0);

    println!();
    println!(
        "Done \u{2014} {countries_with_data} countries with \
         stations, {total_fetched} stations fetched, \
         {total} total in DB"
    );
    println!("Database: {}", db_path().display());
}
