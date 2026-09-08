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
    http::StatusCode,
    response::IntoResponse,
};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::AppState;

const DEFAULT_LIMIT: i64 = 10_000;
const MAX_LIMIT: i64 = 20_000;

pub struct StationsDb {
    conn: Option<Mutex<Connection>>,
}

impl StationsDb {
    pub fn open(path: Option<PathBuf>) -> Self {
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
    Query(query): Query<StationsQuery>,
) -> axum::response::Response {
    let Some(mutex) = state.stations.conn.as_ref() else {
        return unavailable();
    };
    // Hold the guard across both queries so total + page are consistent.
    let conn = mutex.lock().unwrap();
    let (limit, offset) = clamp_paging(&query);
    match query_stations(&conn, query.q.as_deref(), limit, offset) {
        Ok((stations, total)) => Json(StationsResponse {
            stations,
            total,
            limit,
            offset,
        })
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

    #[tokio::test]
    async fn lists_all_without_query() {
        let db = StationsDb::open_memory().unwrap();
        {
            let conn = db.conn.as_ref().unwrap().lock().unwrap();
            seed(&conn);
        }
        let (state, dir) = test_state(db);
        let resp = list_stations(
            State(state),
            Query(StationsQuery {
                q: None,
                limit: None,
                offset: None,
            }),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::OK);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn unconfigured_catalog_answers_503() {
        let (state, dir) = test_state(StationsDb { conn: None });
        let resp = list_stations(
            State(state),
            Query(StationsQuery {
                q: None,
                limit: None,
                offset: None,
            }),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
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
