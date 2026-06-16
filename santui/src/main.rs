use santui_core::Santui;

#[cfg(feature = "radio-streaming-player")]
use santui_ipc::IpcPluginHost;

#[cfg(feature = "auth")]
use santui_auth::AuthClient;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut app = Santui::new();

    #[cfg(feature = "auth")]
    {
        let client_id = std::env::var("SANTUI_GOOGLE_CLIENT_ID").unwrap_or_default();
        let client_secret = std::env::var("SANTUI_GOOGLE_CLIENT_SECRET").unwrap_or_default();
        if !client_id.is_empty() && !client_secret.is_empty() {
            let config = santui_auth::AuthConfig::google(client_id, client_secret);
            let auth: Arc<dyn santui_core::AuthHandle> = Arc::new(AuthClient::new(config));
            app.ctx.auth = Some(auth);
        }
    }

    #[cfg(feature = "radio-streaming-player")]
    app.register(Box::new(IpcPluginHost::new(
        "santui-radio-streaming-player",
        "Radio Streaming Player",
        "santui-radio-streaming-player",
    )));

    app.run()
}
