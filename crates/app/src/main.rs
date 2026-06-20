use santui_core::Santui;
use std::path::PathBuf;

#[cfg(feature = "auth")]
use santui_auth::AuthClient;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut app = Santui::new();

    // Initialize plugin registry (stores config + downloaded plugins in ~/.santui).
    let registry_dir = {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_else(|_| ".".into());
        PathBuf::from(home).join(".santui")
    };
    let config_dir = registry_dir.clone();
    app.set_registry_dir(registry_dir);
    app.set_config_dir(config_dir);

    // Set plugin factory: uses IpcPluginHost for IPC-based plugin binaries.
    app.set_plugin_factory(std::sync::Arc::new(santui_ipc::IpcPluginHost::new_boxed));

    #[cfg(feature = "auth")]
    {
        let client_id = std::env::var("SANTUI_GOOGLE_CLIENT_ID").unwrap_or_default();
        let client_secret = std::env::var("SANTUI_GOOGLE_CLIENT_SECRET").unwrap_or_default();
        if !client_id.is_empty() && !client_secret.is_empty() {
            let config = santui_auth::AuthConfig::google(client_id, client_secret);
            let auth: std::sync::Arc<dyn santui_core::AuthHandle> =
                std::sync::Arc::new(AuthClient::new(config));
            app.ctx.auth = Some(auth);
        }
    }

    app.run()
}
