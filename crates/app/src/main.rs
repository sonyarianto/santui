use std::path::PathBuf;

use santui_core::Santui;
use santui_db::open_db;

#[cfg(feature = "auth")]
use santui_auth::AuthClient;

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
        // Compile-time check: if santui-core is not resolvable this won't compile
        let _ = santui_core::Santui::new();
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
        .format_timestamp(None)
        .format_target(false)
        .try_init();
    let mut app = Santui::new();

    // Initialize local database (per-user key-value storage for plugins).
    let _db = open_db()?;

    // Initialize santui data directory (~/.santui).
    let data_dir = {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_else(|_| ".".into());
        PathBuf::from(home).join(".santui")
    };
    let config_dir = data_dir.clone();
    app.set_data_dir(data_dir);
    app.set_config_dir(config_dir);

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
