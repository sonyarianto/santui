use crate::protocol::ThemeData;

/// A deterministic theme for plugin tests. All render assertions derive their
/// colours from this helper, so tests stay consistent.
pub fn theme() -> ThemeData {
    ThemeData {
        text: [200; 3],
        text_muted: [100; 3],
        accent: [180; 3],
        highlight: [220; 3],
        logo: [255; 3],
        background: [0; 3],
        background_panel: [20; 3],
        background_overlay: [10; 3],
        border: [150; 3],
        success: [80; 3],
        error: [255; 3],
        inverted_text: [255; 3],
    }
}
