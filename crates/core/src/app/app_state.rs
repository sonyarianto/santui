use crate::event::Event;
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
    /// Built-in palette commands: `(id, category, label)`.
    pub builtin_items: Vec<(super::BuiltinId, String, String)>,
    /// Index into `PluginManager::carousel_items()` for the home screen carousel.
    /// `None` means no plugin is selected (bare home screen).
    pub home_selected: Option<usize>,
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
            builtin_items,
            home_selected: None,
        }
    }

    /// Process a batch of events from the EventBus.
    /// Updates internal state in response to theme changes.
    pub fn process_events(&mut self, events: &[Event]) {
        for event in events {
            if let Event::ThemeChanged(theme) = event {
                self.theme = theme.clone();
            }
        }
    }
}
