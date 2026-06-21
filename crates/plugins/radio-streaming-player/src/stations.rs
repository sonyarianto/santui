use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize)]
pub struct Station {
    pub name: String,
    pub url: String,
    pub country: String,
    pub genre: String,
}

pub fn load() -> Vec<Station> {
    match crate::database::open() {
        Ok(conn) => crate::database::load_all(&conn).unwrap_or_default(),
        Err(e) => {
            log::warn!("  ⚠️  SQLite unavailable ({e})");
            Vec::new()
        }
    }
}

pub fn reload() -> Vec<Station> {
    match crate::database::open() {
        Ok(conn) => crate::database::load_all(&conn).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}
