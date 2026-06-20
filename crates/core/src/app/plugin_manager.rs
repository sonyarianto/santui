use crate::plugin::{Plugin, PluginCmdItem, PluginContext};
use crate::theme::Theme;
use crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use ratatui::Frame;

/// Manages the lifecycle, dispatch, and palette-command registry for all
/// loaded plugins.  Extracted from the monolithic `Santui` struct so that
/// Santui itself only owns a single `PluginManager` field.
pub(crate) struct PluginManager {
    plugins: Vec<Box<dyn Plugin>>,
    active_idx: Option<usize>,
    /// Global index → (plugin_index, local_command_index, command).
    plugin_commands: Vec<(usize, usize, PluginCmdItem)>,
}

impl PluginManager {
    pub fn new() -> Self {
        PluginManager {
            plugins: Vec::new(),
            active_idx: None,
            plugin_commands: Vec::new(),
        }
    }

    // ------------------------------------------------------------------
    // Registration & lifecycle
    // ------------------------------------------------------------------

    pub fn register(&mut self, plugin: Box<dyn Plugin>) {
        self.plugins.push(plugin);
    }

    pub fn len(&self) -> usize {
        self.plugins.len()
    }

    pub fn init_all(&mut self, ctx: &mut PluginContext) -> Result<(), Box<dyn std::error::Error>> {
        for p in &mut self.plugins {
            p.init(ctx)?;
        }
        self.refresh_commands();
        Ok(())
    }

    pub fn tick_all(&mut self) {
        for p in &mut self.plugins {
            p.tick();
        }
    }

    /// Register a plugin, initialise it, and return its index.
    pub fn push_and_init(
        &mut self,
        mut plugin: Box<dyn Plugin>,
        ctx: &mut PluginContext,
    ) -> Result<usize, Box<dyn std::error::Error>> {
        plugin.init(ctx)?;
        let idx = self.plugins.len();
        self.plugins.push(plugin);
        Ok(idx)
    }

    // ------------------------------------------------------------------
    // Active plugin
    // ------------------------------------------------------------------

    pub fn active(&self) -> Option<usize> {
        self.active_idx
    }

    pub fn set_active(&mut self, idx: Option<usize>) {
        self.active_idx = idx;
    }

    // ------------------------------------------------------------------
    // Dispatch to a specific plugin
    // ------------------------------------------------------------------

    pub fn handle_key(&mut self, idx: usize, key: KeyEvent) -> bool {
        if idx < self.plugins.len() {
            self.plugins[idx].handle_key(key)
        } else {
            false
        }
    }

    pub fn render(&self, idx: usize, f: &mut Frame, area: Rect) {
        if idx < self.plugins.len() {
            self.plugins[idx].render(f, area);
        }
    }

    pub fn status_hints(&self, idx: usize) -> Vec<(String, String)> {
        if idx < self.plugins.len() {
            self.plugins[idx].status_hints()
        } else {
            vec![]
        }
    }

    pub fn on_blur(&mut self, idx: usize) {
        if idx < self.plugins.len() {
            self.plugins[idx].on_blur();
        }
    }

    /// Dispatch a palette command that originated from the given plugin.
    pub fn handle_palette_command(&mut self, plugin_idx: usize, local_idx: usize) {
        if plugin_idx < self.plugins.len() {
            self.plugins[plugin_idx].handle_palette_command(local_idx);
        }
    }

    // ------------------------------------------------------------------
    // Queries
    // ------------------------------------------------------------------

    pub fn find_by_id(&self, id: &str) -> Option<usize> {
        self.plugins.iter().position(|p| p.id() == id)
    }

    // ------------------------------------------------------------------
    // Broadcasts
    // ------------------------------------------------------------------

    pub fn on_theme_change_all(&mut self, theme: &Theme) {
        for p in &mut self.plugins {
            p.on_theme_change(theme);
        }
    }

    pub fn on_user_update_all(&mut self, user: Option<&crate::auth::User>) {
        for p in &mut self.plugins {
            p.on_user_update(user);
        }
    }

    // ------------------------------------------------------------------
    // Palette commands
    // ------------------------------------------------------------------

    pub fn commands(&self) -> &[(usize, usize, PluginCmdItem)] {
        &self.plugin_commands
    }

    pub fn refresh_commands(&mut self) {
        self.plugin_commands.clear();
        for (i, plugin) in self.plugins.iter().enumerate() {
            for (local_idx, cmd) in plugin.commands().into_iter().enumerate() {
                self.plugin_commands.push((i, local_idx, cmd));
            }
        }
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}
