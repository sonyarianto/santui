use std::path::PathBuf;

use santui_core::{DbAccess, Santui};
use santui_db::open_db;
use santui_registry::Registry;

#[cfg(feature = "auth")]
use santui_auth::AuthClient;

use rusqlite::Connection;

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Wraps the central santui.db connection for plugin key-value access.
struct SantuiDb {
    conn: Option<Connection>,
}

impl DbAccess for SantuiDb {
    fn get_value(&self, plugin: &str, user_id: &str, key: &str) -> Option<String> {
        let conn = self.conn.as_ref()?;
        conn.query_row(
            "SELECT value FROM user_data WHERE plugin = ?1 AND user_id = ?2 AND key = ?3",
            rusqlite::params![plugin, user_id, key],
            |row| row.get::<_, String>(0),
        )
        .ok()
    }

    fn set_value(&mut self, plugin: &str, user_id: &str, key: &str, value: &str) {
        let conn = match self.conn.as_ref() {
            Some(c) => c,
            None => return,
        };
        let _ = conn.execute(
            "INSERT INTO user_data (plugin, user_id, key, value) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(plugin, user_id, key) DO UPDATE SET value = excluded.value",
            rusqlite::params![plugin, user_id, key, value],
        );
    }
}

/// The old data directory location (`~/.santui`) used for one-time migration.
fn old_data_dir() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".santui")
}

/// Migrate files from old `~/.santui` to the platform-standard data directory.
fn migrate_data_dir(old: &std::path::Path, new: &std::path::Path) {
    let entries = match std::fs::read_dir(old) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let src = entry.path();
        let dst = new.join(&name);
        if src.is_dir() {
            if let Err(e) = copy_dir_recursively(&src, &dst) {
                log::warn!("failed to migrate directory {:?}: {e}", name);
            }
        } else if let Err(e) = std::fs::copy(&src, &dst) {
            log::warn!("failed to migrate file {:?}: {e}", name);
        }
    }
}

fn copy_dir_recursively(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_recursively(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

fn list_plugins() -> Result<(), Box<dyn std::error::Error>> {
    let mut reg = Registry::new(santui_db::data_dir());

    println!("santui v{VERSION}");
    println!();

    // Installed plugins
    println!("Installed plugins:");
    if reg.installed.is_empty() {
        println!("  (none)");
    } else {
        for p in &reg.installed {
            let status = if p.enabled { "enabled" } else { "disabled" };
            println!("  {} v{} [{}]", p.name, p.version, status);
            if !p.capabilities.is_empty() {
                println!("    capabilities: {}", p.capabilities.join(", "));
            }
        }
    }

    // Available plugins (best-effort manifest fetch)
    println!();
    match reg.fetch_manifest() {
        Ok(()) => {
            let installed_ids: std::collections::HashSet<&str> =
                reg.installed.iter().map(|p| p.id.as_str()).collect();

            let not_installed: Vec<&santui_registry::PluginManifest> = reg
                .available
                .iter()
                .filter(|m| !installed_ids.contains(m.id.as_str()))
                .collect();

            println!("Available plugins ({} not installed):", not_installed.len());
            if not_installed.is_empty() {
                println!("  (all installed)");
            } else {
                for m in &not_installed {
                    println!("  {} v{} — {}", m.name, m.version, m.description);
                }
            }
        }
        Err(e) => {
            println!("Available plugins:");
            println!("  (could not fetch manifest: {e})");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_santui_crate_version_exists() {
        let version = env!("CARGO_PKG_VERSION");
        assert!(!version.is_empty());
        let parts: Vec<&str> = version.split('.').collect();
        assert_eq!(parts.len(), 3);
    }

    #[test]
    fn test_santui_depends_on_core_crate() {
        let _ = santui_core::Santui::new();
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("santui v{VERSION}");
        return Ok(());
    }

    if args.iter().any(|a| a == "--list-plugins" || a == "plugins") {
        return list_plugins();
    }

    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
        .format_timestamp(None)
        .format_target(false)
        .try_init();
    let mut app = Santui::new();

    // Initialize central database for plugin user data.
    let conn = open_db().ok();
    let db = Box::new(SantuiDb { conn });
    app.set_db(db);

    let dir = santui_db::data_dir();
    let old_dir = old_data_dir();

    // One-time migration from old ~/.santui to platform-standard directory.
    // santui.db and auth-tokens.json already live in the platform-standard dir,
    // so only config, registry, and plugins need migration.
    if old_dir != dir && old_dir.exists() && !dir.join("config.toml").exists() {
        log::info!("migrating data from {:?} to {:?}", old_dir, dir);
        migrate_data_dir(&old_dir, &dir);
    }

    std::fs::create_dir_all(&dir)?;
    if !dir.join("config.toml").exists() {
        santui_core::config::Config::default().save_to(&dir)?;
    }
    app.set_data_dir(dir.clone());
    app.set_config_dir(dir);

    // Set plugin factory: uses IpcPluginHost for IPC-based plugin binaries.
    app.set_plugin_factory(std::sync::Arc::new(santui_ipc::IpcPluginHost::new_boxed));

    // Register the registry plugin (always bundled with santui).
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()));
    let registry_bin = exe_dir
        .as_ref()
        .map(|d| {
            if cfg!(windows) {
                d.join("santui-registry-plugin.exe")
            } else {
                d.join("santui-registry-plugin")
            }
        })
        .unwrap_or_else(|| PathBuf::from("santui-registry-plugin"));
    app.register_default_plugin("plugin-registry", "Plugin Registry", &registry_bin);
    app.set_plugin_persistent("plugin-registry", true);

    #[cfg(feature = "auth")]
    {
        let mut providers = Vec::new();

        let github_id = std::env::var("SANTUI_GITHUB_CLIENT_ID")
            .unwrap_or_else(|_| "Ov23liQ8S6DliNvkWmoB".into());
        let config = santui_auth::AuthConfig::github(github_id);
        providers.push(("github".into(), config));

        let vercel_url = std::env::var("SANTUI_VERCEL_URL")
            .unwrap_or_else(|_| "https://santuiapp.vercel.app".to_string());

        let auth: std::sync::Arc<dyn santui_core::AuthHandle> =
            std::sync::Arc::new(AuthClient::new(providers).with_vercel(vercel_url));
        app.set_auth(auth);
    }

    app.run()
}
