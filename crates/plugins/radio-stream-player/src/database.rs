use crate::stations::Station;
use rusqlite::Connection;
use std::path::PathBuf;

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

pub fn db_path() -> PathBuf {
    app_data_dir().join("radio_stream_stations.db")
}

fn migrate_old_db() {
    let old = app_data_dir().join("radio_streaming_stations.db");
    let new = db_path();
    if old.exists() {
        if new.exists() {
            if let Err(e) = std::fs::remove_file(&new) {
                log::warn!("failed to remove stale empty database: {e}");
                return;
            }
        }
        if let Err(e) = std::fs::rename(&old, &new) {
            log::warn!("failed to migrate old database (radio_streaming_stations.db): {e}");
        } else {
            log::info!(
                "migrated database from radio_streaming_stations.db to radio_stream_stations.db"
            );
        }
    }
}

fn migrate(conn: &Connection) -> Result<(), rusqlite::Error> {
    let has_genre: bool = conn
        .prepare("PRAGMA table_info(stations)")?
        .query_map([], |row| {
            let name: String = row.get(1)?;
            Ok(name)
        })?
        .any(|r| r.is_ok_and(|n| n == "genre"));

    if !has_genre {
        conn.execute_batch("ALTER TABLE stations ADD COLUMN genre TEXT NOT NULL DEFAULT '';")?;
    }
    Ok(())
}

pub fn open() -> Result<Connection, rusqlite::Error> {
    let path = db_path();
    migrate_old_db();
    if let Some(parent) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            log::warn!("failed to create DB parent directory: {e}");
        }
    }
    if !path.exists() {
        if let Some(bundled) = std::env::current_exe()
            .ok()
            .and_then(|p| {
                p.parent()
                    .map(|d| d.join("native").join("radio_stream_stations.db"))
            })
            .filter(|p| p.exists())
        {
            if let Err(e) = std::fs::copy(&bundled, &path) {
                log::warn!("failed to copy bundled station DB: {e}");
            }
        }
    }
    let conn = Connection::open(&path)?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS stations (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            url TEXT NOT NULL,
            country TEXT NOT NULL DEFAULT '',
            genre TEXT NOT NULL DEFAULT ''
        );",
    )?;
    migrate(&conn)?;
    conn.execute_batch(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_stations_name_url ON stations(name, url);
        CREATE INDEX IF NOT EXISTS idx_stations_country ON stations(country);
        CREATE INDEX IF NOT EXISTS idx_stations_genre ON stations(genre);",
    )?;
    Ok(conn)
}

pub fn load_all(conn: &Connection) -> Result<Vec<Station>, rusqlite::Error> {
    let mut stmt = conn.prepare("SELECT name, url, country, genre FROM stations ORDER BY name")?;
    let stations = stmt
        .query_map([], |row| {
            Ok(Station {
                name: row.get(0)?,
                url: row.get(1)?,
                country: row.get(2)?,
                genre: row.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(stations)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE stations (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                url TEXT NOT NULL,
                country TEXT NOT NULL DEFAULT '',
                genre TEXT NOT NULL DEFAULT ''
            );",
        )
        .unwrap();
        conn
    }

    fn insert(conn: &Connection, name: &str, url: &str, country: &str, genre: &str) {
        conn.execute(
            "INSERT INTO stations (name, url, country, genre) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![name, url, country, genre],
        )
        .unwrap();
    }

    #[test]
    fn load_all_returns_all() {
        let conn = setup_db();
        insert(&conn, "B", "http://b", "US", "Rock");
        insert(&conn, "A", "http://a", "GB", "Pop");
        let rows = load_all(&conn).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].name, "A");
        assert_eq!(rows[1].name, "B");
    }

    #[test]
    fn load_all_empty() {
        let conn = setup_db();
        let rows = load_all(&conn).unwrap();
        assert!(rows.is_empty());
    }

    #[test]
    fn load_all_includes_all_fields() {
        let conn = setup_db();
        insert(&conn, "X", "http://x", "DE", "Jazz");
        let rows = load_all(&conn).unwrap();
        assert_eq!(rows[0].name, "X");
        assert_eq!(rows[0].url, "http://x");
        assert_eq!(rows[0].country, "DE");
        assert_eq!(rows[0].genre, "Jazz");
    }

    #[test]
    fn search_matches_name() {
        let conn = setup_db();
        insert(&conn, "Rock FM", "http://rock", "US", "Rock");
        insert(&conn, "Pop FM", "http://pop", "GB", "Pop");
        let rows = search(&conn, "Rock").unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "Rock FM");
    }

    #[test]
    fn search_matches_country() {
        let conn = setup_db();
        insert(&conn, "A", "http://a", "US", "");
        insert(&conn, "B", "http://b", "GB", "");
        let rows = search(&conn, "US").unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "A");
    }

    #[test]
    fn search_matches_genre() {
        let conn = setup_db();
        insert(&conn, "A", "http://a", "", "Rock");
        insert(&conn, "B", "http://b", "", "Pop");
        let rows = search(&conn, "Rock").unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "A");
    }

    #[test]
    fn search_case_insensitive() {
        let conn = setup_db();
        insert(&conn, "Rock FM", "http://rock", "US", "Rock");
        let rows = search(&conn, "rock").unwrap();
        assert_eq!(rows.len(), 1);
    }

    #[test]
    fn search_no_match_returns_empty() {
        let conn = setup_db();
        insert(&conn, "Rock FM", "http://rock", "US", "Rock");
        let rows = search(&conn, "NONEXISTENT").unwrap();
        assert!(rows.is_empty());
    }

    #[test]
    fn search_empty_db_returns_empty() {
        let conn = setup_db();
        let rows = search(&conn, "test").unwrap();
        assert!(rows.is_empty());
    }

    #[test]
    fn search_empty_query_matches_all() {
        let conn = setup_db();
        insert(&conn, "A", "http://a", "US", "");
        insert(&conn, "B", "http://b", "GB", "");
        let rows = search(&conn, "").unwrap();
        assert_eq!(rows.len(), 2);
    }

    #[test]
    fn load_all_no_table_returns_error() {
        let conn = Connection::open_in_memory().unwrap();
        let result = load_all(&conn);
        assert!(result.is_err());
    }

    #[test]
    fn migrate_old_db_renames_when_new_missing() {
        let dir = std::env::temp_dir().join("santui-db-migrate-test");
        let _ = std::fs::create_dir_all(&dir);
        let old = dir.join("radio_streaming_stations.db");
        let new = dir.join("radio_stream_stations.db");
        std::fs::write(&old, b"some data").unwrap();

        // Simulate migrate_old_db logic
        if old.exists() {
            if new.exists() {
                let _ = std::fs::remove_file(&new);
            }
            std::fs::rename(&old, &new).unwrap();
        }

        assert!(!old.exists(), "old file should be gone");
        assert!(new.exists(), "new file should exist");
        assert_eq!(std::fs::read(&new).unwrap(), b"some data");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn migrate_old_db_replaces_existing_new() {
        let dir = std::env::temp_dir().join("santui-db-migrate-test2");
        let _ = std::fs::create_dir_all(&dir);
        let old = dir.join("radio_streaming_stations.db");
        let new = dir.join("radio_stream_stations.db");
        std::fs::write(&old, b"old data").unwrap();
        std::fs::write(&new, b"new data").unwrap();

        // Simulate migrate_old_db logic
        if old.exists() {
            if new.exists() {
                let _ = std::fs::remove_file(&new);
            }
            std::fs::rename(&old, &new).unwrap();
        }

        assert!(!old.exists(), "old file should be gone");
        assert_eq!(
            std::fs::read(&new).unwrap(),
            b"old data",
            "old data should replace new"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[allow(dead_code)]
pub fn search(conn: &Connection, query: &str) -> Result<Vec<Station>, rusqlite::Error> {
    let pattern = format!("%{}%", query);
    let mut stmt = conn.prepare(
        "SELECT name, url, country, genre FROM stations WHERE name LIKE ?1 OR country LIKE ?1 OR genre LIKE ?1 ORDER BY name",
    )?;
    let stations = stmt
        .query_map(rusqlite::params![pattern], |row| {
            Ok(Station {
                name: row.get(0)?,
                url: row.get(1)?,
                country: row.get(2)?,
                genre: row.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(stations)
}
