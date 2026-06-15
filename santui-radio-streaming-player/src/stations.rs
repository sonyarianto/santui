use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize)]
pub struct Station {
    pub name: String,
    pub url: String,
    pub country: String,
}

const EMBEDDED_JSON: &str = include_str!("../radio_streaming_stations.json");

pub fn embedded_stations() -> Vec<Station> {
    serde_json::from_str(EMBEDDED_JSON).expect("embedded stations.json is valid")
}

pub fn load() -> Vec<Station> {
    match crate::database::open() {
        Ok(mut conn) => {
            let _ = crate::database::seed_if_empty(&mut conn);
            crate::database::load_all(&conn).unwrap_or_else(|_| embedded_stations())
        }
        Err(e) => {
            eprintln!("  ⚠️  SQLite unavailable ({e}), using embedded stations");
            embedded_stations()
        }
    }
}

pub fn reload() -> Vec<Station> {
    match crate::database::open() {
        Ok(conn) => crate::database::load_all(&conn).unwrap_or_else(|_| embedded_stations()),
        Err(_) => embedded_stations(),
    }
}
