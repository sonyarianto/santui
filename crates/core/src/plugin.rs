use crate::auth::{AuthHandle, User};
use crate::theme::Theme;
use crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use ratatui::Frame;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Factory that creates a `Box<dyn Plugin>` from an id, name, and binary path.
/// The binary (`santui`) sets this to `IpcPluginHost::new_boxed`.
pub type PluginFactory = Arc<dyn Fn(&str, &str, &Path) -> Box<dyn Plugin + Send> + Send + Sync>;

/// A command that a plugin registers for the Ctrl+P palette.
#[derive(Debug, Clone)]
pub struct PluginCmdItem {
    pub category: String,
    pub label: String,
}

pub trait Plugin: Send {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    fn init(&mut self, ctx: &mut PluginContext) -> Result<(), Box<dyn std::error::Error>>;
    fn handle_key(&mut self, _key: KeyEvent) -> bool {
        false
    }
    fn render(&self, _f: &mut Frame, _area: Rect) {}
    fn tick(&mut self) {}
    fn on_focus(&mut self) {}
    fn on_blur(&mut self) {}
    fn on_theme_change(&mut self, _theme: &Theme) {}
    fn on_user_update(&mut self, _user: Option<&User>) {}
    fn status_hints(&self) -> Vec<(String, String)> {
        vec![]
    }
    /// Palette commands that this plugin registers (category, label).
    fn commands(&self) -> Vec<PluginCmdItem> {
        vec![]
    }
    /// Called when a palette command from this plugin is selected.
    /// `index` is the position in `commands()`.
    fn handle_palette_command(&mut self, _index: usize) {}

    /// Called when a plugin-to-plugin message arrives.
    fn on_plugin_message(&mut self, _from: &str, _action: &str, _data: &str) {}

    /// Called before the plugin is unloaded (hot-reload or app exit).
    /// The plugin should flush state, close handles, and prepare to be dropped.
    /// For IPC plugins this sends `Shutdown` and waits briefly for a response
    /// before the child process is killed.
    fn shutdown(&mut self) {}

    /// Return the filesystem path to this plugin's binary, if it runs as an
    /// external process.  `None` for in-process plugins.  Used by the hot-reload
    /// mechanism to detect when the binary has been updated on disk.
    fn binary_path(&self) -> Option<&Path> {
        None
    }

    /// Whether the plugin process is still running.
    /// Returns `true` for in-process plugins.
    /// IPC plugins override this to check the child process.
    fn is_alive(&self) -> bool {
        true
    }

    /// Whether the plugin supports running in the background when the user
    /// presses Esc (e.g., audio players that should keep playing).
    /// Default is `false`. Override to return `true` for background-capable plugins.
    fn can_background(&self) -> bool {
        false
    }
}

pub struct PluginContext {
    pub theme: Theme,
    pub auth: Option<Arc<dyn AuthHandle>>,
    /// Santui data directory (e.g. `~/.santui`). Plugins can use this
    /// for persistent storage. The registry plugin uses it to find
    /// installed plugins and `registry.toml`.
    pub data_dir: PathBuf,
}

impl PluginContext {
    pub fn new() -> Self {
        PluginContext {
            theme: Theme::default(),
            auth: None,
            data_dir: PathBuf::new(),
        }
    }
}

impl Default for PluginContext {
    fn default() -> Self {
        Self::new()
    }
}
