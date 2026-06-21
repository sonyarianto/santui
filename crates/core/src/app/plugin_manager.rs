use std::path::Path;
use std::time::SystemTime;

use crate::event::Event;
use crate::plugin::{Plugin, PluginCmdItem, PluginContext, PluginFactory};
use crate::theme::Theme;
use crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use ratatui::Frame;
use santui_registry::Registry as PluginRegistry;

/// Manages the lifecycle, dispatch, and palette-command registry for all
/// loaded plugins.  Extracted from the monolithic `Santui` struct so that
/// Santui itself only owns a single `PluginManager` field.
pub(crate) struct PluginManager {
    plugins: Vec<Box<dyn Plugin + Send>>,
    active_idx: Option<usize>,
    /// Global index → (plugin_index, local_command_index, command).
    plugin_commands: Vec<(usize, usize, PluginCmdItem)>,
    /// Factory for recreating plugins during hot-reload.
    plugin_factory: Option<PluginFactory>,
    /// Last known modification times for each plugin's binary, parallel to
    /// `plugins`.  `None` for in-process plugins or when stat failed.
    mtimes: Vec<Option<SystemTime>>,
    /// Dynamic palette items from enabled registry plugins: (category, plugin_id, name).
    dynamic_items: Vec<(String, String, String)>,
}

impl PluginManager {
    pub fn new() -> Self {
        PluginManager {
            plugins: Vec::new(),
            active_idx: None,
            plugin_commands: Vec::new(),
            plugin_factory: None,
            mtimes: Vec::new(),
            dynamic_items: Vec::new(),
        }
    }

    /// Store the plugin factory so we can recreate plugins during hot-reload.
    pub fn set_factory(&mut self, factory: PluginFactory) {
        self.plugin_factory = Some(factory);
    }

    // ------------------------------------------------------------------
    // Registration & lifecycle
    // ------------------------------------------------------------------

    pub fn register(&mut self, plugin: Box<dyn Plugin + Send>) {
        self.mtimes.push(stat_mtime(plugin.binary_path()));
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

    /// Use the internal factory to create a plugin from (id, name, binary_path),
    /// initialise it, register it, and return its index.
    pub fn spawn_and_init(
        &mut self,
        id: &str,
        name: &str,
        binary_path: &std::path::Path,
        ctx: &mut PluginContext,
    ) -> Result<usize, Box<dyn std::error::Error>> {
        let factory = self
            .plugin_factory
            .as_ref()
            .ok_or_else::<Box<dyn std::error::Error>, _>(|| "no factory".into())?;
        let mut plugin = factory(id, name, binary_path);
        plugin.init(ctx)?;
        let idx = self.plugins.len();
        self.mtimes.push(stat_mtime(plugin.binary_path()));
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
    // Hot-reload
    // ------------------------------------------------------------------

    /// Check every plugin's binary mtime and reload any that have changed.
    /// Called once per frame from the event loop.
    pub fn check_reloads(&mut self, ctx: &mut PluginContext) {
        for idx in 0..self.plugins.len() {
            if let Err(e) = self.reload_plugin(idx, ctx) {
                let name = self.plugins[idx].name().to_string();
                log::error!("[santui] Failed to reload plugin `{name}`: {e}");
            }
        }
    }

    /// Reload the plugin at `idx` if its binary has changed on disk.
    /// In-process plugins (no binary path) are skipped.
    fn reload_plugin(
        &mut self,
        idx: usize,
        ctx: &mut PluginContext,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = match self.plugins[idx].binary_path() {
            Some(p) => p.to_path_buf(),
            None => return Ok(()),
        };

        let factory = match self.plugin_factory.as_ref() {
            Some(f) => f,
            None => return Ok(()),
        };

        let current_mtime = stat_mtime(Some(&path));
        let stored = self.mtimes.get(idx).copied().flatten();

        if current_mtime == stored {
            return Ok(());
        }

        // Binary changed — recreate the plugin.
        let id = self.plugins[idx].id().to_string();
        let name = self.plugins[idx].name().to_string();

        let mut new_plugin = factory(&id, &name, &path);
        new_plugin.init(ctx)?;

        // Update stored mtime *before* replacing so a second consecutive poll
        // doesn't trigger another reload.
        if idx < self.mtimes.len() {
            self.mtimes[idx] = current_mtime;
        }

        // Gracefully shut down the old plugin before dropping it.
        self.plugins[idx].shutdown();
        self.plugins[idx] = new_plugin;
        self.refresh_commands();

        Ok(())
    }

    // ------------------------------------------------------------------
    // Broadcasts
    // ------------------------------------------------------------------

    pub fn on_theme_change_all(&mut self, theme: &Theme) {
        for p in &mut self.plugins {
            p.on_theme_change(theme);
        }
    }

    /// Process a batch of events from the EventBus.
    pub fn process_events(&mut self, events: &[Event]) {
        for event in events {
            match event {
                Event::PluginMessage {
                    from,
                    to,
                    action,
                    data,
                } => {
                    if let Some(idx) = self.find_by_id(to) {
                        self.plugins[idx].on_plugin_message(from, action, data);
                    }
                }
                Event::ThemeChanged(theme) => {
                    self.on_theme_change_all(theme);
                }
                Event::UserUpdated => {}
            }
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

    pub fn dynamic_items(&self) -> &[(String, String, String)] {
        &self.dynamic_items
    }

    /// Rebuild dynamic palette items from the plugin registry.
    /// Iterates installed plugins directly so that enabled modules appear
    /// even before the remote manifest is fetched. The display name is read
    /// from the manifest if available, otherwise derived from the binary name.
    pub fn refresh_dynamic_items(&mut self, registry: &Option<PluginRegistry>) {
        self.dynamic_items.clear();
        if let Some(ref reg) = registry {
            for installed in &reg.installed {
                if !installed.enabled {
                    continue;
                }
                let id = if !installed.id.is_empty() {
                    installed.id.clone()
                } else {
                    match installed
                        .path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .map(|s| s.trim_end_matches(".exe").to_string())
                    {
                        Some(id) => id,
                        None => continue,
                    }
                };
                let name = if !installed.name.is_empty() {
                    installed.name.clone()
                } else {
                    reg.available
                        .iter()
                        .find(|m| m.id == *id)
                        .map(|m| m.name.clone())
                        .unwrap_or_else(|| {
                            // Humanize: "santui-radio-streaming-player" → "Radio Streaming Player"
                            id.trim_start_matches("santui-")
                                .replace('-', " ")
                                .split_ascii_whitespace()
                                .map(|w| {
                                    let mut c = w.chars();
                                    match c.next() {
                                        None => String::new(),
                                        Some(f) => f.to_uppercase().to_string() + c.as_str(),
                                    }
                                })
                                .collect::<Vec<_>>()
                                .join(" ")
                        })
                };
                if !self
                    .dynamic_items
                    .iter()
                    .any(|(_, existing_id, _)| *existing_id == id)
                {
                    self.dynamic_items
                        .push(("Plugins".into(), id.clone(), name));
                }
            }
        }
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper: resolve `SystemTime` from an optional path.
fn stat_mtime(path: Option<&Path>) -> Option<SystemTime> {
    path.and_then(|p| std::fs::metadata(p).ok())
        .and_then(|m| m.modified().ok())
}
