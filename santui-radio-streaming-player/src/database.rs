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

pub fn open() -> Result<Connection, rusqlite::Error> {
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

pub fn seed_if_empty(conn: &mut Connection) -> Result<(), rusqlite::Error> {
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM stations", [], |row| row.get(0))?;
    if count > 0 {
        return Ok(());
    }
    let stations = crate::stations::embedded_stations();
    let tx = conn.transaction()?;
    for s in &stations {
        tx.execute(
            "INSERT INTO stations (name, url, country) VALUES (?1, ?2, ?3)",
            rusqlite::params![s.name, s.url, s.country],
        )?;
    }
    tx.commit()?;
    Ok(())
}

pub fn load_all(conn: &Connection) -> Result<Vec<Station>, rusqlite::Error> {
    let mut stmt = conn.prepare("SELECT name, url, country FROM stations ORDER BY name")?;
    let stations = stmt
        .query_map([], |row| {
            Ok(Station {
                name: row.get(0)?,
                url: row.get(1)?,
                country: row.get(2)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(stations)
}

#[allow(dead_code)]
pub fn search(conn: &Connection, query: &str) -> Result<Vec<Station>, rusqlite::Error> {
    let pattern = format!("%{}%", query);
    let mut stmt = conn.prepare(
        "SELECT name, url, country FROM stations WHERE name LIKE ?1 OR country LIKE ?1 ORDER BY name",
    )?;
    let stations = stmt
        .query_map(rusqlite::params![pattern], |row| {
            Ok(Station {
                name: row.get(0)?,
                url: row.get(1)?,
                country: row.get(2)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(stations)
}
