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
    app_data_dir().join("radio_streaming_stations.db")
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
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if !path.exists() {
        if let Some(bundled) = std::env::current_exe()
            .ok()
            .and_then(|p| {
                p.parent()
                    .map(|d| d.join("native").join("radio_streaming_stations.db"))
            })
            .filter(|p| p.exists())
        {
            let _ = std::fs::copy(&bundled, &path);
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
