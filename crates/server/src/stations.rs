//! Public radio station catalog.
//!
//! Read-only view over a copy of `radio_stream_stations.db` (same schema as
//! the desktop plugin's local database). The catalog file is optional:
//! configure it with `--stations-db` / `SANTUI_STATIONS_DB`. When it is
//! missing the endpoints answer 503 so the rest of the server keeps working.
//!
//! Response shape mirrors the desktop plugin's `Station` struct field names
//! (`name`, `url`, `country`, `genre`) — keep them in sync.

use axum::{
    Json,
    extract::{Query, State},
    http::{HeaderMap, StatusCode, header},
    response::IntoResponse,
};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::AppState;

const DEFAULT_LIMIT: i64 = 10_000;
const MAX_LIMIT: i64 = 20_000;

pub struct StationsDb {
    conn: Option<Mutex<Connection>>,
    /// Strong ETag for the catalog file (`"<mtime>-<size>"`), computed once at
    /// startup. The catalog only changes when the file is replaced (redeploy),
    /// so file metadata is a sufficient version signal — no content scan.
    etag: Option<String>,
}

/// ETag for a catalog file. `None` when metadata is unreadable.
fn file_etag(path: &Path) -> Option<String> {
    let meta = std::fs::metadata(path).ok()?;
    let mtime = meta
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();
    Some(format!("\"{mtime:x}-{len:x}\"", len = meta.len()))
}

/// Normalize an `If-None-Match` header value for comparison: strip the weak
/// `W/` prefix. Supports a single tag or `*` (handled by the caller).
fn normalize_inm(value: &str) -> &str {
    value.trim().strip_prefix("W/").unwrap_or(value.trim())
}

fn etag_matches(header_value: &str, etag: &str) -> bool {
    header_value
        .split(',')
        .any(|tag| normalize_inm(tag) == etag || normalize_inm(tag) == "*")
}

impl StationsDb {
    /// Empty catalog (no database). Used when unconfigured and in tests.
    pub fn empty() -> Self {
        StationsDb {
            conn: None,
            etag: None,
        }
    }

    pub fn open(path: Option<PathBuf>) -> Self {
        let etag = path.as_deref().and_then(file_etag);
        let conn = path
            .filter(|p| p.exists())
            .and_then(|p| Connection::open(p).ok());
        if conn.is_none() {
            tracing::warn!(
                "stations catalog not configured (set --stations-db / SANTUI_STATIONS_DB); /api/v1/stations will answer 503"
            );
        }
        StationsDb {
            conn: conn.map(Mutex::new),
            etag,
        }
    }

    #[cfg(test)]
    fn open_memory() -> Result<Self, rusqlite::Error> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(
            "CREATE TABLE stations (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                url TEXT NOT NULL,
                country TEXT NOT NULL DEFAULT '',
                genre TEXT NOT NULL DEFAULT ''
            );",
        )?;
        Ok(StationsDb {
            conn: Some(Mutex::new(conn)),
            etag: None,
        })
    }
}

#[derive(Clone, Serialize)]
pub struct Station {
    pub name: String,
    pub url: String,
    pub country: String,
    pub genre: String,
}

#[derive(Deserialize)]
pub struct StationsQuery {
    q: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
}

#[derive(Serialize)]
pub struct StationsResponse {
    pub stations: Vec<Station>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

fn clamp_paging(query: &StationsQuery) -> (i64, i64) {
    let limit = query.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
    let offset = query.offset.unwrap_or(0).max(0);
    (limit, offset)
}

/// Escape LIKE metacharacters so `q` is always a literal substring search.
fn like_pattern(q: &str) -> String {
    let escaped: String = q
        .chars()
        .flat_map(|c| match c {
            '%' | '_' | '\\' => vec!['\\', c],
            _ => vec![c],
        })
        .collect();
    format!("%{escaped}%")
}

fn map_station(row: &rusqlite::Row) -> rusqlite::Result<Station> {
    Ok(Station {
        name: row.get(0)?,
        url: row.get(1)?,
        country: row.get(2)?,
        genre: row.get(3)?,
    })
}

fn query_stations(
    conn: &Connection,
    q: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<(Vec<Station>, i64), rusqlite::Error> {
    let (where_clause, pattern) = match q.filter(|s| !s.is_empty()) {
        Some(q) => (
            "WHERE name LIKE ?1 ESCAPE '\\' OR country LIKE ?1 ESCAPE '\\' OR genre LIKE ?1 ESCAPE '\\'",
            Some(like_pattern(q)),
        ),
        None => ("", None),
    };
    let total: i64 = if let Some(ref p) = pattern {
        conn.query_row(
            &format!("SELECT COUNT(*) FROM stations {where_clause}"),
            rusqlite::params![p],
            |row| row.get(0),
        )?
    } else {
        conn.query_row("SELECT COUNT(*) FROM stations", [], |row| row.get(0))?
    };

    let sql = match pattern {
        Some(_) => format!(
            "SELECT name, url, country, genre FROM stations {where_clause} ORDER BY name LIMIT ?2 OFFSET ?3"
        ),
        None => "SELECT name, url, country, genre FROM stations ORDER BY name LIMIT ?1 OFFSET ?2"
            .to_string(),
    };
    let mut stmt = conn.prepare(&sql)?;
    let rows = match pattern {
        Some(p) => stmt.query_map(rusqlite::params![p, limit, offset], map_station)?,
        None => stmt.query_map(rusqlite::params![limit, offset], map_station)?,
    };
    let mut stations = Vec::new();
    for row in rows {
        stations.push(row?);
    }
    Ok((stations, total))
}

fn unavailable() -> axum::response::Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(serde_json::json!({"error": "stations catalog not configured"})),
    )
        .into_response()
}

pub async fn list_stations(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<StationsQuery>,
) -> axum::response::Response {
    let Some(mutex) = state.stations.conn.as_ref() else {
        return unavailable();
    };
    // Hold the guard across both queries so total + page are consistent.
    let conn = mutex.lock().unwrap();
    let etag_headers = etag_header_map(state.stations.etag.as_deref());

    // Conditional request: catalog unchanged since the client's copy.
    if let Some(etag) = state.stations.etag.as_deref()
        && let Some(inm) = headers.get(header::IF_NONE_MATCH)
        && let Ok(inm) = inm.to_str()
        && etag_matches(inm, etag)
    {
        return (StatusCode::NOT_MODIFIED, etag_headers).into_response();
    }

    let (limit, offset) = clamp_paging(&query);
    match query_stations(&conn, query.q.as_deref(), limit, offset) {
        Ok((stations, total)) => (
            StatusCode::OK,
            etag_headers,
            Json(StationsResponse {
                stations,
                total,
                limit,
                offset,
            }),
        )
            .into_response(),
        Err(e) => {
            tracing::error!("stations query error: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "stations query failed"})),
            )
                .into_response()
        }
    }
}

/// `ETag` response headers, or empty when the catalog is unconfigured.
fn etag_header_map(etag: Option<&str>) -> HeaderMap {
    let mut headers = HeaderMap::new();
    if let Some(etag) = etag
        && let Ok(value) = axum::http::HeaderValue::from_str(etag)
    {
        headers.insert(header::ETAG, value);
    }
    headers
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ServerConfig;

    fn test_state(db: StationsDb) -> (Arc<AppState>, PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "santui-srv-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let state = Arc::new(AppState {
            config: ServerConfig {
                port: 0,
                host: String::new(),
                data_dir: PathBuf::new(),
                jwt_secret: String::new(),
                stations_db: None,
                google_client_id: None,
            },
            db: crate::db::Database::open(&dir).unwrap(),
            stations: db,
        });
        (state, dir)
    }

    fn seed(conn: &Connection) {
        conn.execute_batch(
            "INSERT INTO stations (name, url, country, genre) VALUES
             ('Rock FM', 'http://rock', 'US', 'Rock'),
             ('Pop FM', 'http://pop', 'GB', 'Pop'),
             ('Jazz 24', 'http://jazz', 'US', 'Jazz');",
        )
        .unwrap();
    }

    fn no_query() -> Query<StationsQuery> {
        Query(StationsQuery {
            q: None,
            limit: None,
            offset: None,
        })
    }

    fn inm_headers(value: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::IF_NONE_MATCH,
            axum::http::HeaderValue::from_str(value).unwrap(),
        );
        headers
    }

    #[tokio::test]
    async fn lists_all_without_query() {
        let db = StationsDb::open_memory().unwrap();
        {
            let conn = db.conn.as_ref().unwrap().lock().unwrap();
            seed(&conn);
        }
        let (state, dir) = test_state(db);
        let resp = list_stations(State(state), HeaderMap::new(), no_query()).await;
        assert_eq!(resp.status(), StatusCode::OK);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn unconfigured_catalog_answers_503() {
        let (state, dir) = test_state(StationsDb {
            conn: None,
            etag: None,
        });
        let resp = list_stations(State(state), HeaderMap::new(), no_query()).await;
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn etag_served_and_honored() {
        let mut db = StationsDb::open_memory().unwrap();
        {
            let conn = db.conn.as_ref().unwrap().lock().unwrap();
            seed(&conn);
        }
        db.etag = Some("\"abc-123\"".to_string());
        let (state, dir) = test_state(db);

        let resp = list_stations(State(state.clone()), HeaderMap::new(), no_query()).await;
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(resp.headers().get(header::ETAG).unwrap(), "\"abc-123\"");

        for inm in [
            "\"abc-123\"",
            "W/\"abc-123\"",
            "*",
            "\"other\", \"abc-123\"",
        ] {
            let resp = list_stations(State(state.clone()), inm_headers(inm), no_query()).await;
            assert_eq!(resp.status(), StatusCode::NOT_MODIFIED, "inm={inm}");
            assert!(resp.headers().contains_key(header::ETAG));
        }

        let resp = list_stations(State(state.clone()), inm_headers("\"stale\""), no_query()).await;
        assert_eq!(resp.status(), StatusCode::OK);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn paging_is_clamped() {
        assert_eq!(
            clamp_paging(&StationsQuery {
                q: None,
                limit: Some(0),
                offset: Some(-5)
            }),
            (1, 0)
        );
        assert_eq!(
            clamp_paging(&StationsQuery {
                q: None,
                limit: Some(999_999),
                offset: None
            }),
            (MAX_LIMIT, 0)
        );
        assert_eq!(
            clamp_paging(&StationsQuery {
                q: None,
                limit: None,
                offset: None
            }),
            (DEFAULT_LIMIT, 0)
        );
    }

    #[test]
    fn like_pattern_escapes_wildcards() {
        assert_eq!(like_pattern("100%"), "%100\\%%");
        assert_eq!(like_pattern("a_b"), "%a\\_b%");
        assert_eq!(like_pattern("plain"), "%plain%");
    }

    #[test]
    fn file_etag_is_stable_and_missing_is_none() {
        let dir = std::env::temp_dir();
        let path = dir.join("santui-etag-probe.db");
        std::fs::write(&path, b"data").unwrap();
        let a = file_etag(&path);
        let b = file_etag(&path);
        assert!(a.is_some());
        assert_eq!(a, b);
        assert!(file_etag(&dir.join("santui-etag-missing.db")).is_none());
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn etag_matches_weak_wildcard_and_lists() {
        assert!(etag_matches("\"abc\"", "\"abc\""));
        assert!(etag_matches("W/\"abc\"", "\"abc\""));
        assert!(etag_matches("*", "\"abc\""));
        assert!(etag_matches("\"x\", \"abc\"", "\"abc\""));
        assert!(!etag_matches("\"stale\"", "\"abc\""));
        assert!(!etag_matches("", "\"abc\""));
    }

    #[test]
    fn query_filters_and_paginates() {
        let db = StationsDb::open_memory().unwrap();
        let conn = db.conn.as_ref().unwrap().lock().unwrap();
        seed(&conn);
        let (all, total) = query_stations(&conn, None, 10, 0).unwrap();
        assert_eq!(total, 3);
        assert_eq!(all.len(), 3);
        // ORDER BY name: Jazz 24, Pop FM, Rock FM
        assert_eq!(all[0].name, "Jazz 24");

        let (page, total) = query_stations(&conn, None, 2, 1).unwrap();
        assert_eq!(total, 3);
        assert_eq!(page.len(), 2);
        assert_eq!(page[0].name, "Pop FM");

        let (found, total) = query_stations(&conn, Some("rock"), 10, 0).unwrap();
        assert_eq!(total, 1);
        assert_eq!(found[0].name, "Rock FM");

        let (found, _) = query_stations(&conn, Some("US"), 10, 0).unwrap();
        assert_eq!(found.len(), 2);

        // Literal % must not act as a wildcard.
        let (found, total) = query_stations(&conn, Some("100%"), 10, 0).unwrap();
        assert_eq!(total, 0);
        assert!(found.is_empty());
    }

    #[test]
    fn query_on_missing_table_errors() {
        let conn = Connection::open_in_memory().unwrap();
        assert!(query_stations(&conn, None, 10, 0).is_err());
    }
}
