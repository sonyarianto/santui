use rusqlite::Connection;
use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize)]
pub struct Station {
    pub name: String,
    pub url: String,
    pub country: String,
    pub genre: String,
}

pub fn load(conn: &Connection) -> Vec<Station> {
    crate::database::load_all(conn).unwrap_or_else(|e| {
        log::warn!("  ⚠️  SQLite load_all failed ({e})");
        Vec::new()
    })
}

pub fn reload(conn: &Connection) -> Vec<Station> {
    crate::database::load_all(conn).unwrap_or_default()
}
