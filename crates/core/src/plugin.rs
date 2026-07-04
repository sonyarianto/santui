use crate::auth::{AuthHandle, User};
use crate::db_access::DbAccess;
use crate::theme::Theme;
use crossterm::event::{KeyEvent, MouseEvent};
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
    fn handle_mouse(&mut self, _event: &MouseEvent) -> bool {
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
    /// For IPC plugins this sends `Shutdown` (non-blocking) — the child process
    /// is killed later in `Drop`/`kill()` when the channel senders are dropped.
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

    /// Whether the plugin has completed initialization and is ready to render.
    /// In-process plugins are ready immediately. IPC plugins become ready once
    /// the child process responds to the `Init` message.
    fn is_ready(&self) -> bool {
        true
    }

    /// Whether the plugin's last render included a dim overlay (search palette, dialog, etc.).
    /// The host uses this to extend the dim to the status bar.
    fn has_dim_overlay(&self) -> bool {
        false
    }

    /// Whether the plugin supports running in the background when the user
    /// presses Esc (e.g., audio players that should keep playing).
    /// Default is `false`. Override to return `true` for background-capable plugins.
    fn can_background(&self) -> bool {
        false
    }

    /// Set runtime capabilities (e.g. `"background"`) declared in the plugin manifest.
    /// Default is no-op; IPC plugins override this to store the value.
    fn set_capabilities(&mut self, _caps: Vec<String>) {}

    /// Whether this plugin should stay loaded when the user presses Esc.
    /// Persistent plugins are blurred but not shut down, keeping their
    /// palette entries and background processes alive.
    /// Default is `false`. Override to return `true` for plugins that
    /// must not be unloaded (e.g., the registry plugin).
    fn persistent(&self) -> bool {
        false
    }

    /// Mark this plugin as persistent.  Default is no-op; IPC plugins
    /// override this to store the value.
    fn set_persistent(&mut self, _persistent: bool) {}

    /// Check if a pending Esc key response has been resolved.
    /// Returns `Some(consumed)` if resolved, `None` if still pending
    /// or no Esc was sent.
    /// IPC plugins override this; in-process plugins don't need it.
    fn take_pending_esc_result(&mut self) -> Option<bool> {
        None
    }

    /// Process any pending requests from the plugin (e.g. DB read/write,
    /// authentication). Called once per main loop iteration.
    /// The default implementation is a no-op. IPC plugins override this
    /// to forward `PluginRequest` variants to the host.
    fn process_pending_requests(
        &mut self,
        _db: &mut dyn DbAccess,
        _auth: Option<&Arc<dyn AuthHandle>>,
    ) {
    }

    /// Drain any pending launch request from this plugin.
    /// Returns `Some((id, name))` if a launch was requested, `None` otherwise.
    fn drain_pending_launch(&mut self) -> Option<(String, String)> {
        None
    }
}

pub struct PluginContext {
    pub theme: Theme,
    pub auth: Option<Arc<dyn AuthHandle>>,
    /// Santui data directory (platform-standard, e.g. `%APPDATA%/santui` on Windows). Plugins can use this
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
