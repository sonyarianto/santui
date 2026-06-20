use crate::theme::Theme;

/// Centralized application state.
///
/// Single source of truth for UI-level flags, the active theme, and
/// built-in command definitions.
#[derive(Debug)]
pub struct AppState {
    /// Whether the main event loop should keep running.
    pub running: bool,
    /// Whether the about screen is shown.
    pub show_about: bool,
    /// The currently active theme.
    pub theme: Theme,
    /// Whether the theme picker overlay is open.
    pub theme_picker_open: bool,
    /// Whether the plugin registry overlay is open.
    pub registry_open: bool,
    /// Built-in palette commands: `(id, category, label)`.
    pub builtin_items: Vec<(super::BuiltinId, String, String)>,
}

impl AppState {
    pub fn new(theme: Theme) -> Self {
        let builtin_items = super::all_builtins()
            .into_iter()
            .map(|(id, cat, label)| (id, cat.to_string(), label.to_string()))
            .collect();
        AppState {
            running: true,
            show_about: false,
            theme,
            theme_picker_open: false,
            registry_open: false,
            builtin_items,
        }
    }
}
