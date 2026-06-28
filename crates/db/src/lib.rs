use rusqlite::{Connection, Result};
use std::path::PathBuf;

fn data_dir() -> PathBuf {
    if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local").join("share"))
            })
            .unwrap_or_else(|| PathBuf::from("."))
    }
    .join("santui")
}

fn db_path() -> PathBuf {
    data_dir().join("santui.db")
}

pub fn open_db() -> Result<Connection> {
    let path = db_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let conn = Connection::open(&path)?;
    create_schema(&conn)?;
    Ok(conn)
}

fn create_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch("PRAGMA journal_mode=WAL;")?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS user_data (
            plugin  TEXT NOT NULL,
            user_id TEXT NOT NULL,
            key     TEXT NOT NULL,
            value   TEXT NOT NULL,
            PRIMARY KEY (plugin, user_id, key)
        );",
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mem_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        create_schema(&conn).unwrap();
        conn
    }

    // ─── Schema tests ───

    #[test]
    fn test_schema_creates_user_data_table() {
        let conn = mem_db();
        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='user_data'")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        assert_eq!(tables, vec!["user_data"]);
    }

    #[test]
    fn test_schema_sets_wal_journal_mode() {
        let tmp = std::env::temp_dir().join("santui-db-wal-test.db");
        let _ = std::fs::remove_file(&tmp);
        let conn = Connection::open(&tmp).unwrap();
        create_schema(&conn).unwrap();
        let mode: String = conn
            .pragma_query_value(None, "journal_mode", |row| row.get(0))
            .unwrap();
        assert_eq!(mode.to_lowercase(), "wal");
        drop(conn);
        std::fs::remove_file(&tmp).ok();
    }

    #[test]
    fn test_schema_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        create_schema(&conn).unwrap();
        create_schema(&conn).unwrap();
    }

    // ─── CRUD tests ───

    #[test]
    fn test_insert_and_read() {
        let conn = mem_db();
        conn.execute(
            "INSERT INTO user_data (plugin, user_id, key, value) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params!["radio", "user1", "favorite_station", "102.7"],
        )
        .unwrap();
        let val: String = conn
            .query_row(
                "SELECT value FROM user_data WHERE plugin=?1 AND user_id=?2 AND key=?3",
                rusqlite::params!["radio", "user1", "favorite_station"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(val, "102.7");
    }

    #[test]
    fn test_upsert_updates_existing() {
        let conn = mem_db();
        conn.execute(
            "INSERT INTO user_data (plugin, user_id, key, value) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params!["radio", "u1", "volume", "50"],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO user_data (plugin, user_id, key, value) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(plugin, user_id, key) DO UPDATE SET value=excluded.value",
            rusqlite::params!["radio", "u1", "volume", "75"],
        )
        .unwrap();
        let val: String = conn
            .query_row(
                "SELECT value FROM user_data WHERE plugin=?1 AND user_id=?2 AND key=?3",
                rusqlite::params!["radio", "u1", "volume"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(val, "75");
    }

    #[test]
    fn test_primary_key_rejects_duplicate() {
        let conn = mem_db();
        conn.execute(
            "INSERT INTO user_data (plugin, user_id, key, value) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params!["p", "u", "k", "v1"],
        )
        .unwrap();
        let err = conn.execute(
            "INSERT INTO user_data (plugin, user_id, key, value) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params!["p", "u", "k", "v2"],
        );
        assert!(err.is_err(), "duplicate PK should be rejected");
    }

    #[test]
    fn test_select_missing_key_returns_error() {
        let conn = mem_db();
        let val: Result<String> = conn.query_row(
            "SELECT value FROM user_data WHERE plugin=?1 AND user_id=?2 AND key=?3",
            rusqlite::params!["p", "u", "nonexistent"],
            |row| row.get(0),
        );
        assert!(val.is_err());
    }

    #[test]
    fn test_isolation_different_users() {
        let conn = mem_db();
        conn.execute(
            "INSERT INTO user_data (plugin, user_id, key, value) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params!["p", "alice", "theme", "dark"],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO user_data (plugin, user_id, key, value) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params!["p", "bob", "theme", "light"],
        )
        .unwrap();
        let alice: String = conn
            .query_row(
                "SELECT value FROM user_data WHERE plugin=?1 AND user_id=?2 AND key=?3",
                rusqlite::params!["p", "alice", "theme"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(alice, "dark");
        let bob: String = conn
            .query_row(
                "SELECT value FROM user_data WHERE plugin=?1 AND user_id=?2 AND key=?3",
                rusqlite::params!["p", "bob", "theme"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(bob, "light");
    }

    #[test]
    fn test_isolation_different_plugins() {
        let conn = mem_db();
        conn.execute(
            "INSERT INTO user_data (plugin, user_id, key, value) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params!["radio", "u1", "volume", "80"],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO user_data (plugin, user_id, key, value) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params!["todo", "u1", "volume", "40"],
        )
        .unwrap();
        let radio: String = conn
            .query_row(
                "SELECT value FROM user_data WHERE plugin=?1 AND user_id=?2 AND key=?3",
                rusqlite::params!["radio", "u1", "volume"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(radio, "80");
        let todo: String = conn
            .query_row(
                "SELECT value FROM user_data WHERE plugin=?1 AND user_id=?2 AND key=?3",
                rusqlite::params!["todo", "u1", "volume"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(todo, "40");
    }

    #[test]
    fn test_multiple_keys_per_user() {
        let conn = mem_db();
        for (k, v) in [("volume", "60"), ("station", "89.5 FM"), ("genre", "jazz")] {
            conn.execute(
                "INSERT INTO user_data (plugin, user_id, key, value) VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params!["radio", "u1", k, v],
            )
            .unwrap();
        }
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM user_data WHERE plugin=?1 AND user_id=?2",
                rusqlite::params!["radio", "u1"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 3);
    }

    #[test]
    fn test_delete_single_key() {
        let conn = mem_db();
        conn.execute(
            "INSERT INTO user_data (plugin, user_id, key, value) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params!["radio", "u1", "k1", "v1"],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO user_data (plugin, user_id, key, value) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params!["radio", "u1", "k2", "v2"],
        )
        .unwrap();
        conn.execute(
            "DELETE FROM user_data WHERE plugin=?1 AND user_id=?2 AND key=?3",
            rusqlite::params!["radio", "u1", "k1"],
        )
        .unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM user_data WHERE plugin=?1 AND user_id=?2",
                rusqlite::params!["radio", "u1"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_delete_all_for_user() {
        let conn = mem_db();
        conn.execute(
            "INSERT INTO user_data (plugin, user_id, key, value) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params!["radio", "u1", "a", "1"],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO user_data (plugin, user_id, key, value) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params!["radio", "u1", "b", "2"],
        )
        .unwrap();
        conn.execute(
            "DELETE FROM user_data WHERE plugin=?1 AND user_id=?2",
            rusqlite::params!["radio", "u1"],
        )
        .unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM user_data WHERE plugin=?1 AND user_id=?2",
                rusqlite::params!["radio", "u1"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_delete_unaffected_other_users() {
        let conn = mem_db();
        conn.execute(
            "INSERT INTO user_data (plugin, user_id, key, value) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params!["p", "alice", "k", "alice_val"],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO user_data (plugin, user_id, key, value) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params!["p", "bob", "k", "bob_val"],
        )
        .unwrap();
        conn.execute(
            "DELETE FROM user_data WHERE plugin=?1 AND user_id=?2",
            rusqlite::params!["p", "alice"],
        )
        .unwrap();
        let bob_val: String = conn
            .query_row(
                "SELECT value FROM user_data WHERE plugin=?1 AND user_id=?2 AND key=?3",
                rusqlite::params!["p", "bob", "k"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(bob_val, "bob_val");
    }

    #[test]
    fn test_json_value_roundtrip() {
        let conn = mem_db();
        let data = serde_json::json!({"stations": ["89.5", "102.7"], "volume": 75});
        let json_str = serde_json::to_string(&data).unwrap();
        conn.execute(
            "INSERT INTO user_data (plugin, user_id, key, value) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params!["radio", "u1", "prefs", &json_str],
        )
        .unwrap();
        let stored: String = conn
            .query_row(
                "SELECT value FROM user_data WHERE plugin=?1 AND user_id=?2 AND key=?3",
                rusqlite::params!["radio", "u1", "prefs"],
                |row| row.get(0),
            )
            .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&stored).unwrap();
        assert_eq!(parsed["volume"], 75);
        assert_eq!(parsed["stations"][0], "89.5");
    }

    #[test]
    fn test_upsert_preserves_other_keys() {
        let conn = mem_db();
        conn.execute(
            "INSERT INTO user_data (plugin, user_id, key, value) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params!["p", "u", "k1", "v1"],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO user_data (plugin, user_id, key, value) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(plugin, user_id, key) DO UPDATE SET value=excluded.value",
            rusqlite::params!["p", "u", "k2", "v2"],
        )
        .unwrap();
        let k1: String = conn
            .query_row(
                "SELECT value FROM user_data WHERE plugin=?1 AND user_id=?2 AND key=?3",
                rusqlite::params!["p", "u", "k1"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(k1, "v1");
        let k2: String = conn
            .query_row(
                "SELECT value FROM user_data WHERE plugin=?1 AND user_id=?2 AND key=?3",
                rusqlite::params!["p", "u", "k2"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(k2, "v2");
    }

    // ─── data_dir / db_path path logic ───

    #[test]
    fn test_data_dir_ends_with_santui() {
        assert_eq!(data_dir().file_name().unwrap(), "santui");
    }

    #[test]
    fn test_db_path_ends_with_santui_db() {
        let path = db_path();
        assert_eq!(path.file_name().unwrap(), "santui.db");
        assert_eq!(path.parent().unwrap(), &data_dir());
    }
}
