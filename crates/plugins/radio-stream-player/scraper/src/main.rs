//! Scrapes internet radio stations from an online directory
//! and saves them to the Santui radio stream player's SQLite database.
//!
//! ## Usage
//!
//! ```bash
//! cargo run -p santui-radio-stream-scraper
//! ```
//!
//! Fetches the currently-playing station list from every country via the
//! `?nowlisten=1` endpoint. Deduplicates by `(name, url)` using `UNIQUE`
//! constraint + `INSERT OR IGNORE`. Run periodically to keep stations fresh.
//!
//! The database is shared with the radio plugin at:
//! - **Windows**: `%APPDATA%\santui\radio_stream_stations.db`
//! - **Linux/macOS**: `~/.local/share/santui/radio_stream_stations.db`
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
    app_data_dir().join("radio_stream_stations.db")
}

/// Returns true if a column exists on the stations table.
fn has_column(conn: &Connection, col: &str) -> Result<bool, rusqlite::Error> {
    Ok(conn
        .prepare("PRAGMA table_info(stations)")?
        .query_map([], |row| {
            let name: String = row.get(1)?;
            Ok(name)
        })?
        .any(|r| r.is_ok_and(|n| n == col)))
}

fn migrate(conn: &Connection) -> Result<(), rusqlite::Error> {
    if !has_column(conn, "genre")? {
        conn.execute_batch("ALTER TABLE stations ADD COLUMN genre TEXT NOT NULL DEFAULT '';")?;
    }
    if !has_column(conn, "radio_id")? {
        conn.execute_batch("ALTER TABLE stations ADD COLUMN radio_id TEXT NOT NULL DEFAULT '';")?;
    }
    Ok(())
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
            country TEXT NOT NULL DEFAULT '',
            genre TEXT NOT NULL DEFAULT '',
            radio_id TEXT NOT NULL DEFAULT ''
        );",
    )?;
    migrate(&conn)?;
    conn.execute_batch(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_stations_name_url ON stations(name, url);
        CREATE INDEX IF NOT EXISTS idx_stations_country ON stations(country);
        CREATE INDEX IF NOT EXISTS idx_stations_genre ON stations(genre);
        CREATE INDEX IF NOT EXISTS idx_stations_genre_null ON stations(genre) WHERE genre = '';",
    )?;
    Ok(conn)
}

fn extract_stations(html: &str) -> Vec<(String, String, String)> {
    let mut stations = Vec::new();
    let search = r#"class="b-play station_play"#;
    let mut pos = 0;
    while let Some(start) = html[pos..].find(search) {
        let fragment_start = pos + start;
        let fragment = &html[fragment_start..];

        let stream = extract_attr(fragment, "stream");
        let name = extract_attr(fragment, "radioName");
        let radio_id = extract_attr(fragment, "radioId");

        if let (Some(url), Some(name)) = (stream, name) {
            let name = unescape_html(&name);
            if !url.is_empty() && !name.is_empty() {
                let url = url
                    .replace("?dist=onlineradiobox", "")
                    .replace("&dist=onlineradiobox", "")
                    .replace("?ref=onlineradiobox26", "")
                    .replace("&ref=onlineradiobox26", "");
                stations.push((name, url, radio_id.unwrap_or_default()));
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

fn fetch_country_http(url_code: &str) -> Result<Vec<(String, String, String)>, String> {
    let url = format!("https://onlineradiobox.com/{url_code}/?nowlisten=1");
    let mut resp = ureq::get(&url)
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .header("Accept", "application/json, text/plain, */*")
        .call()
        .map_err(|e| format!("request failed: {e}"))?;
    let body = resp
        .body_mut()
        .read_to_string()
        .map_err(|e| format!("failed to read response: {e}"))?;

    let resp: RadioBoxResponse =
        serde_json::from_str(&body).map_err(|e| format!("JSON parse error: {e}"))?;

    Ok(extract_stations(&resp.data))
}

/// How many station-genre pages to fetch per scraper run.
const GENRE_SAMPLE_SIZE: usize = 50;

/// Parse genre names out of a station detail page HTML that contains:
/// ```html
/// <ul class="station__tags" role="list">
///   <li><a href="...">pop</a></li>
///   ...
/// </ul>
/// ```
fn parse_genres(html: &str) -> Vec<String> {
    let mut genres = Vec::new();
    let search = r#"<ul class="station__tags""#;
    let Some(tag_start) = html.find(search) else {
        return genres;
    };
    let ul_fragment = &html[tag_start..];
    let Some(close_start) = ul_fragment.find("</ul>") else {
        return genres;
    };
    let inner = &ul_fragment[..close_start];

    let mut pos = 0;
    let li_marker = "<li><a href=\"";
    while let Some(link_start) = inner[pos..].find(li_marker) {
        let value_start = pos + link_start + li_marker.len();
        let after_href = &inner[value_start..];
        let Some(quote_end) = after_href.find('"') else {
            break;
        };
        let after_close = &after_href[quote_end..];
        let Some(gt_pos) = after_close.find('>') else {
            break;
        };
        let genre_start = gt_pos + 1;
        let genre_text = &after_close[genre_start..];
        let Some(close_a) = genre_text.find("</a>") else {
            break;
        };
        let genre = genre_text[..close_a].trim();
        if !genre.is_empty() {
            genres.push(genre.to_string());
        }
        pos = value_start + quote_end + after_close[quote_end..].find("</a>").unwrap_or(0) + 4;
    }
    genres
}

/// How many already-genred stations to refresh per run.
const REFRESH_PER_RUN: usize = 10;

/// Fetch a station's detail page and parse genre tags (HTTP only, no DB).
/// radio_id is `us.kmgl` — converted to `https://onlineradiobox.com/us/kmgl/`.
/// Returns the genres string (comma-separated) on success.
fn fetch_and_parse_genre(radio_id: &str) -> Result<String, String> {
    let path = radio_id.replace('.', "/");
    let url = format!("https://onlineradiobox.com/{path}/");
    let mut resp = ureq::get(&url)
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .call()
        .map_err(|e| format!("request failed: {e}"))?;
    let body = resp
        .body_mut()
        .read_to_string()
        .map_err(|e| format!("failed to read response: {e}"))?;

    let genres = parse_genres(&body);
    if genres.is_empty() {
        return Ok(String::new());
    }
    Ok(genres.join(", "))
}

fn pick_radio_ids(conn: &Connection, where_clause: &str, limit: usize) -> Vec<String> {
    let sql = format!(
        "SELECT radio_id FROM stations WHERE radio_id != '' AND {where_clause} ORDER BY RANDOM() LIMIT ?1"
    );
    let mut stmt = conn.prepare(&sql).expect("prepare failed");
    stmt.query_map(rusqlite::params![limit as i64], |row| row.get(0))
        .expect("query failed")
        .filter_map(|r| r.ok())
        .collect()
}

/// Enrich a random sample of stations — fill new and refresh existing.
/// HTTP fetches run concurrently, DB updates are serial.
fn enrich_genres(conn: &Connection) {
    let new_limit = GENRE_SAMPLE_SIZE - REFRESH_PER_RUN;
    let mut rows: Vec<(String, String)> = pick_radio_ids(conn, "genre = ''", new_limit)
        .into_iter()
        .map(|id| (id, "new".to_string()))
        .collect();

    let refresh_limit = GENRE_SAMPLE_SIZE - rows.len();
    if refresh_limit > 0 {
        let refreshes = pick_radio_ids(conn, "genre != ''", refresh_limit);
        rows.extend(refreshes.into_iter().map(|id| (id, "refresh".to_string())));
    }

    if rows.is_empty() {
        println!("No stations with radio_id to enrich.");
        return;
    }

    let num_workers: usize = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);

    println!(
        "\nEnriching {} stations ({} new, {} refresh, {} workers)...",
        rows.len(),
        rows.iter().filter(|(_, t)| t == "new").count(),
        rows.iter().filter(|(_, t)| t == "refresh").count(),
        num_workers,
    );

    let (tx, rx) = std::sync::mpsc::channel::<(String, String, Result<String, String>)>();

    let chunk_size = rows.len().div_ceil(num_workers);
    std::thread::scope(|s| {
        for chunk in rows.chunks(chunk_size) {
            let tx = tx.clone();
            let chunk: Vec<(String, String)> = chunk.to_vec();
            s.spawn(move || {
                for (radio_id, tag) in &chunk {
                    let result = fetch_and_parse_genre(radio_id);
                    let _ = tx.send((radio_id.clone(), tag.clone(), result));
                }
            });
        }
        drop(tx);

        let mut enriched = 0usize;
        let mut failed = 0usize;
        for (radio_id, tag, result) in rx {
            match result {
                Ok(genres) => {
                    if !genres.is_empty() {
                        let _ = conn.execute(
                            "UPDATE stations SET genre = ?1 WHERE radio_id = ?2",
                            rusqlite::params![genres, radio_id],
                        );
                    }
                    enriched += 1;
                    println!("  📡  [{tag}] {radio_id}: {genres}");
                }
                Err(e) => {
                    log::warn!("  ⚠️  {radio_id}: {e}");
                    failed += 1;
                }
            }
        }
        println!("Genre enrichment done — {enriched} enriched, {failed} failed");
    });
}

fn main() {
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
        .format_timestamp(None)
        .format_target(false)
        .try_init();
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
            log::error!("Failed to open database: {e}");
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

    if let Err(e) = conn.execute_batch("BEGIN TRANSACTION") {
        log::error!("Failed to begin transaction: {e}");
        std::process::exit(1);
    }

    let (tx, rx) = std::sync::mpsc::channel::<(String, String, Vec<(String, String, String)>)>();
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
                            log::warn!("  \u{26a0}\u{fe0f}  {url_code}: {e}");
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
            for (name, url, radio_id) in &stations {
                match conn.execute(
                    "INSERT OR IGNORE INTO stations (name, url, country, genre, radio_id) VALUES (?1, ?2, ?3, ?4, ?5)",
                    rusqlite::params![name, url, iso_code, "", radio_id],
                ) {
                    Ok(rows) => {
                        if rows > 0 {
                            inserted += 1;
                        }
                    }
                    Err(e) => {
                        log::error!("  \u{26a0}\u{fe0f}  insert error for {name}: {e}");
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

    if let Err(e) = conn.execute_batch("COMMIT") {
        log::error!("Failed to commit transaction: {e}");
        std::process::exit(1);
    }

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

    enrich_genres(&conn);
}
