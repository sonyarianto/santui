use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::event::Event;
use crate::plugin::{Plugin, PluginCmdItem, PluginContext, PluginFactory};
use crate::theme::Theme;
use crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use ratatui::Frame;
use serde::{Deserialize, Serialize};

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
    /// Santui data directory (~/.santui). Set from main.rs.
    data_dir: PathBuf,
    /// Last mtime of registry.toml, used for change detection.
    registry_mtime: Option<SystemTime>,
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
            data_dir: PathBuf::new(),
            registry_mtime: None,
        }
    }

    /// Store the santui data directory (~/.santui).
    pub fn set_data_dir(&mut self, dir: PathBuf) {
        self.data_dir = dir;
    }

    /// Get the santui data directory.
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
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

    /// Create and register a plugin from the factory without initialising it.
    /// The plugin will be initialised during the next `init_all` call.
    pub fn register_new(&mut self, id: &str, name: &str, path: &Path) {
        if let Some(ref factory) = self.plugin_factory {
            let plugin = factory(id, name, path);
            self.register(plugin);
        }
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

    /// Poll `registry.toml` for changes and update dynamic palette items.
    /// Called once per frame. Returns true if items changed.
    pub fn poll_registry_installed(&mut self) -> bool {
        let path = self.data_dir.join("registry.toml");
        let current_mtime = match std::fs::metadata(&path) {
            Ok(m) => m.modified().ok(),
            Err(_) => {
                self.dynamic_items.clear();
                return false;
            }
        };

        if current_mtime == self.registry_mtime {
            return false;
        }
        self.registry_mtime = current_mtime;
        self.read_registry_installed()
    }

    /// Re-read `registry.toml` and rebuild `dynamic_items`.
    /// Returns true if items changed.
    pub fn read_registry_installed(&mut self) -> bool {
        let path = self.data_dir.join("registry.toml");
        let cfg = RegistryConfig::load(&path);
        let old = std::mem::take(&mut self.dynamic_items);

        if let Some(cfg) = cfg {
            for installed in &cfg.plugins {
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

        self.dynamic_items != old
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}

/// A plugin the user has installed (either enabled or disabled).
/// Mirrors the type in `santui-registry` so core can read `registry.toml`
/// without depending on that crate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct InstalledPlugin {
    pub enabled: bool,
    pub version: String,
    pub path: PathBuf,
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
}

/// Registry configuration file (`registry.toml`) contents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RegistryConfig {
    pub plugins: Vec<InstalledPlugin>,
}

impl RegistryConfig {
    pub fn load(path: &Path) -> Option<Self> {
        let text = std::fs::read_to_string(path).ok()?;
        toml::from_str(&text).ok()
    }
}

/// An item in the home-screen carousel.
/// Represents a plugin that is either already loaded or available in the registry.
#[derive(Debug, Clone)]
pub struct CarouselItem {
    pub id: String,
    pub name: String,
    /// Index into `self.plugins` if the plugin is already loaded.
    pub plugin_idx: Option<usize>,
}

impl PluginManager {
    /// Build a unified carousel list from loaded plugins + enabled registry items.
    ///
    /// Order:
    /// 1. All currently loaded plugins (in registration order).
    /// 2. Enabled registry plugins whose id is not already loaded (deduplicated).
    pub fn carousel_items(&self) -> Vec<CarouselItem> {
        let mut items: Vec<CarouselItem> = self
            .plugins
            .iter()
            .enumerate()
            .map(|(i, p)| CarouselItem {
                id: p.id().to_string(),
                name: p.name().to_string(),
                plugin_idx: Some(i),
            })
            .collect();

        // Append registry items that aren't already loaded.
        let loaded_ids: HashSet<&str> = self.plugins.iter().map(|p| p.id()).collect();
        for (_, id, name) in &self.dynamic_items {
            if !loaded_ids.contains(id.as_str()) {
                items.push(CarouselItem {
                    id: id.clone(),
                    name: name.clone(),
                    plugin_idx: None,
                });
            }
        }

        items
    }
}

/// Helper: resolve `SystemTime` from an optional path.
fn stat_mtime(path: Option<&Path>) -> Option<SystemTime> {
    path.and_then(|p| std::fs::metadata(p).ok())
        .and_then(|m| m.modified().ok())
}
