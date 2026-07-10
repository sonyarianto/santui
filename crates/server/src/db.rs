use rusqlite::{params, Connection, Result};
use std::path::Path;
use std::sync::Mutex;

pub struct Database {
    conn: Mutex<Connection>,
}

fn create_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch("PRAGMA journal_mode=WAL;")?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS users (
            id         TEXT PRIMARY KEY,
            provider   TEXT NOT NULL,
            email      TEXT NOT NULL DEFAULT '',
            name       TEXT NOT NULL DEFAULT '',
            avatar_url TEXT,
            created_at TEXT NOT NULL DEFAULT (strftime('%s', 'now'))
        );

        CREATE TABLE IF NOT EXISTS user_data (
            plugin     TEXT NOT NULL,
            user_id    TEXT NOT NULL REFERENCES users(id),
            key        TEXT NOT NULL,
            value      TEXT NOT NULL,
            updated_at TEXT NOT NULL DEFAULT (strftime('%s', 'now')),
            PRIMARY KEY (plugin, user_id, key)
        );",
    )?;
    Ok(())
}

impl Database {
    pub fn open(dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(dir).ok();
        let path = dir.join("santui-server.db");
        let conn = Connection::open(&path)?;
        create_schema(&conn)?;
        Ok(Database {
            conn: Mutex::new(conn),
        })
    }

    pub fn get_user(&self, user_id: &str) -> Result<Option<UserRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, provider, email, name, avatar_url, created_at FROM users WHERE id = ?1",
        )?;
        let mut rows = stmt.query(params![user_id])?;
        match rows.next()? {
            Some(row) => Ok(Some(UserRow {
                id: row.get(0)?,
                provider: row.get(1)?,
                email: row.get(2)?,
                name: row.get(3)?,
                avatar_url: row.get(4)?,
                created_at: row.get(5)?,
            })),
            None => Ok(None),
        }
    }

    pub fn upsert_user(&self, user: &UserRow) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO users (id, provider, email, name, avatar_url)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(id) DO UPDATE SET
                email = excluded.email,
                name = excluded.name,
                avatar_url = excluded.avatar_url",
            params![
                user.id,
                user.provider,
                user.email,
                user.name,
                user.avatar_url,
            ],
        )?;
        Ok(())
    }

    pub fn list_values(
        &self,
        plugin: &str,
        user_id: &str,
        since: Option<i64>,
    ) -> Result<Vec<DataRow>> {
        let conn = self.conn.lock().unwrap();
        let (sql, param_values): (String, Vec<Box<dyn rusqlite::types::ToSql>>) =
            if let Some(ts) = since {
                let sql = "SELECT plugin, user_id, key, value, updated_at FROM user_data \
                       WHERE plugin = ?1 AND user_id = ?2 AND CAST(updated_at AS INTEGER) > ?3"
                    .to_string();
                (
                    sql,
                    vec![
                        Box::new(plugin.to_string()),
                        Box::new(user_id.to_string()),
                        Box::new(ts),
                    ],
                )
            } else {
                let sql = "SELECT plugin, user_id, key, value, updated_at FROM user_data \
                       WHERE plugin = ?1 AND user_id = ?2"
                    .to_string();
                (
                    sql,
                    vec![Box::new(plugin.to_string()), Box::new(user_id.to_string())],
                )
            };

        let mut stmt = conn.prepare(&sql)?;
        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();
        let rows = stmt.query_map(param_refs.as_slice(), |row| {
            Ok(DataRow {
                plugin: row.get(0)?,
                user_id: row.get(1)?,
                key: row.get(2)?,
                value: row.get(3)?,
                updated_at: row.get(4)?,
            })
        })?;

        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    pub fn upsert_value(&self, plugin: &str, user_id: &str, key: &str, value: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO user_data (plugin, user_id, key, value, updated_at)
             VALUES (?1, ?2, ?3, ?4, strftime('%s', 'now'))
             ON CONFLICT(plugin, user_id, key) DO UPDATE SET
                value = excluded.value,
                updated_at = strftime('%s', 'now')",
            params![plugin, user_id, key, value],
        )?;
        Ok(())
    }

    pub fn delete_value(&self, plugin: &str, user_id: &str, key: &str) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let affected = conn.execute(
            "DELETE FROM user_data WHERE plugin = ?1 AND user_id = ?2 AND key = ?3",
            params![plugin, user_id, key],
        )?;
        Ok(affected > 0)
    }

    pub fn ensure_user(&self, user_id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO users (id, provider, email, name)
             VALUES (?1, 'internal', '', '')",
            params![user_id],
        )?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct UserRow {
    pub id: String,
    pub provider: String,
    pub email: String,
    pub name: String,
    pub avatar_url: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DataRow {
    pub plugin: String,
    pub user_id: String,
    pub key: String,
    pub value: String,
    pub updated_at: String,
}
