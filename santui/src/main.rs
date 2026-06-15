use santui_core::Santui;

#[cfg(feature = "radio-streaming-player")]
use santui_ipc::IpcPluginHost;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut app = Santui::new();

    #[cfg(feature = "radio-streaming-player")]
    app.register(Box::new(IpcPluginHost::new(
        "santui-radio-streaming-player",
        "Radio Streaming Player",
        "santui-radio-streaming-player",
    )));

    app.run()
}
