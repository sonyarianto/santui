use santui_core::Santui;
use santui_radio_streaming_player::RadioPlugin;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut app = Santui::new();
    app.register(Box::new(RadioPlugin::new()));
    app.run()
}
