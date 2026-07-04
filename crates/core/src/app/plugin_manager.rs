use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::auth::AuthHandle;
use crate::db_access::DbAccess;
use crate::event::Event;
use crate::plugin::{Plugin, PluginCmdItem, PluginContext, PluginFactory};
use crate::registry_config::RegistryConfig;
use crate::theme::Theme;
use crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use ratatui::Frame;
use std::sync::Arc;

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
    /// Capabilities declared in registry.toml, keyed by plugin id.
    plugin_capabilities: HashMap<String, Vec<String>>,
    /// Plugin ids that were installed via the registry (from registry.toml).
    /// Used by `reap_unregistered` to avoid killing built-in plugins.
    registry_plugin_ids: HashSet<String>,
    /// Santui data directory (platform-standard). Set from main.rs.
    data_dir: PathBuf,
    /// Last mtime of registry.toml, used for change detection.
    registry_mtime: Option<SystemTime>,
    /// Throttle: only stat plugin binaries every N frames.
    reload_skip: u32,
    /// Throttle: only stat registry.toml every N frames.
    registry_poll_skip: u32,
    /// Names of plugins that have crashed since last check.
    crashed_plugins: Vec<String>,
    /// Cached carousel items, rebuilt on demand.
    carousel_cache: Option<Vec<CarouselItem>>,
    /// Pending launch requests collected from plugins (id, name).
    pending_launches: Vec<(String, String)>,
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
            plugin_capabilities: HashMap::new(),
            registry_plugin_ids: HashSet::new(),
            data_dir: PathBuf::new(),
            registry_mtime: None,
            reload_skip: 0,
            registry_poll_skip: 0,
            crashed_plugins: Vec::new(),
            carousel_cache: None,
            pending_launches: Vec::new(),
        }
    }

    /// Store the santui data directory.
    pub fn set_data_dir(&mut self, dir: PathBuf) {
        self.data_dir = dir;
    }

    /// Get the santui data directory.
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    /// Store the plugin factory so we can recreate plugins during hot-reload.
    pub fn set_persistent(&mut self, id: &str, persistent: bool) {
        if let Some(plugin) = self.plugins.iter_mut().find(|p| p.id() == id) {
            plugin.set_persistent(persistent);
        }
    }

    pub fn set_factory(&mut self, factory: PluginFactory) {
        self.plugin_factory = Some(factory);
    }

    // ------------------------------------------------------------------
    // Registration & lifecycle
    // ------------------------------------------------------------------

    pub fn register(&mut self, plugin: Box<dyn Plugin + Send>) {
        self.mtimes.push(stat_mtime(plugin.binary_path()));
        self.plugins.push(plugin);
        self.invalidate_carousel_cache();
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
        self.crashed_plugins.clear();
        for p in &mut self.plugins {
            p.tick();
            if !p.is_alive() {
                self.crashed_plugins.push(p.name().to_string());
            }
        }
    }

    /// Process any pending `PluginRequest`s for all plugins.
    /// Called once per main loop iteration (after tick and after key events).
    pub fn process_all_requests(
        &mut self,
        db: &mut dyn DbAccess,
        auth: &Option<Arc<dyn AuthHandle>>,
    ) {
        let auth_ref = auth.as_ref();
        for p in &mut self.plugins {
            p.process_pending_requests(db, auth_ref);
            if let Some(launch) = p.drain_pending_launch() {
                self.pending_launches.push(launch);
            }
        }
    }

    /// Drain all pending launch requests collected from plugins.
    /// Returns a list of `(id, name)` pairs.
    pub fn drain_pending_launches(&mut self) -> Vec<(String, String)> {
        std::mem::take(&mut self.pending_launches)
    }

    pub fn crashed_plugins(&self) -> &[String] {
        &self.crashed_plugins
    }

    /// Use the internal factory to create a plugin from (id, name, binary_path),
    /// initialise it, register it, and return its index.
    pub fn spawn_and_init(
        &mut self,
        id: &str,
        name: &str,
        binary_path: &std::path::Path,
        capabilities: &[String],
        ctx: &mut PluginContext,
    ) -> Result<usize, Box<dyn std::error::Error>> {
        validate_binary_path(binary_path, &self.data_dir)?;
        let factory = self
            .plugin_factory
            .as_ref()
            .ok_or_else::<Box<dyn std::error::Error>, _>(|| "no factory".into())?;
        let mut plugin = factory(id, name, binary_path);
        plugin.set_capabilities(capabilities.to_vec());
        plugin.init(ctx)?;
        let idx = self.plugins.len();
        self.mtimes.push(stat_mtime(plugin.binary_path()));
        self.plugins.push(plugin);
        self.invalidate_carousel_cache();
        Ok(idx)
    }

    // ------------------------------------------------------------------
    // Active plugin
    // ------------------------------------------------------------------

    pub fn active(&self) -> Option<usize> {
        self.active_idx
    }

    pub fn set_active(&mut self, idx: Option<usize>) {
        if self.active_idx == idx {
            return;
        }
        if let Some(old) = self.active_idx {
            if old < self.plugins.len() {
                self.plugins[old].on_blur();
            }
        }
        self.active_idx = idx;
        if let Some(new) = idx {
            if new < self.plugins.len() {
                self.plugins[new].on_focus();
            }
        }
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

    /// Check if a pending Esc response has been resolved for plugin `idx`.
    /// Returns `Some(consumed)` if resolved, `None` if still pending.
    pub fn drain_esc_result(&mut self, idx: usize) -> Option<bool> {
        if idx < self.plugins.len() {
            self.plugins[idx].take_pending_esc_result()
        } else {
            None
        }
    }

    /// Shut down an IPC plugin and remove it from the managed set.
    ///
    /// Background-capable plugins (e.g., audio players) are kept alive — only
    /// blurred and removed from the active display.
    ///
    /// In-process plugins (no binary path) are *not* removed — only the active
    /// index is cleared so the home screen regains focus.
    pub fn shutdown_and_remove(&mut self, idx: usize) {
        if idx >= self.plugins.len() {
            return;
        }

        // Persistent plugins (e.g., registry) stay loaded so their palette
        // entries and background processes survive.
        if self.plugins[idx].persistent() {
            self.plugins[idx].on_blur();
            self.active_idx = None;
            return;
        }

        // Background-capable plugins: blur but keep alive.
        if self.plugins[idx].can_background() {
            self.plugins[idx].on_blur();
            self.active_idx = None;
            return;
        }

        // Only remove out-of-process plugins; keep in-process plugins loaded.
        if self.plugins[idx].binary_path().is_some() {
            self.plugins[idx].on_blur();
            self.plugins[idx].shutdown();
            self.plugins.remove(idx);
            self.mtimes.remove(idx);
            self.refresh_commands();

            // Adjust active_idx for the shift caused by removal.
            // Without this, an active_idx > idx becomes a stale/out-of-bounds
            // pointer after the Vec compacts.
            if let Some(active) = self.active_idx {
                if active > idx {
                    self.active_idx = Some(active - 1);
                } else if active == idx {
                    self.active_idx = None;
                }
            }
            self.invalidate_carousel_cache();
            return;
        }

        if self.active_idx == Some(idx) {
            self.active_idx = None;
        }
        self.invalidate_carousel_cache();
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
        self.reload_skip = self.reload_skip.saturating_sub(1);
        if self.reload_skip > 0 {
            return;
        }
        self.reload_skip = 30;
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

        // Shut down the old plugin first so no two plugin processes are
        // alive at the same time, preventing an orphan if kill() fails.
        self.plugins[idx].shutdown();
        // Drop the old plugin now (kills child process) before spawning a
        // new one.  If init() fails below, at worst we are left without a
        // running plugin (the palette entry still exists).
        self.plugins[idx] = factory(&id, &name, &path);
        // Restore capabilities from the cached map.
        if let Some(caps) = self.plugin_capabilities.get(&id) {
            self.plugins[idx].set_capabilities(caps.clone());
        }
        // Clear stale commands before refreshing.
        self.mtimes[idx] = None;

        // Update stored mtime *before* init, so if init fails the mtime is
        // still bumped and we don't retry on every frame.
        if let Some(mtime) = current_mtime {
            if idx < self.mtimes.len() {
                self.mtimes[idx] = Some(mtime);
            }
        }

        self.plugins[idx].init(ctx)?;
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

    // ------------------------------------------------------------------
    // Crash recovery
    // ------------------------------------------------------------------

    /// Recreate a plugin that has crashed using the stored factory.
    /// Returns an error if the plugin is in-process (no binary path) or
    /// no factory is registered.
    pub fn restart_plugin(
        &mut self,
        idx: usize,
        ctx: &mut PluginContext,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = match self.plugins[idx].binary_path() {
            Some(p) => p.to_path_buf(),
            None => return Err("cannot restart in-process plugin".into()),
        };
        let factory = self
            .plugin_factory
            .as_ref()
            .ok_or_else::<Box<dyn std::error::Error>, _>(|| "no factory".into())?;
        let id = self.plugins[idx].id().to_string();
        let name = self.plugins[idx].name().to_string();

        self.plugins[idx].shutdown();
        self.plugins[idx] = factory(&id, &name, &path);
        if let Some(caps) = self.plugin_capabilities.get(&id) {
            self.plugins[idx].set_capabilities(caps.clone());
        }
        self.mtimes[idx] = stat_mtime(Some(&path));
        self.plugins[idx].init(ctx)?;
        self.refresh_commands();
        log::info!("[santui] Restarted plugin `{name}`");
        Ok(())
    }

    /// Restart all plugins that have crashed.  Returns the number of
    /// successfully restarted plugins.
    pub fn restart_crashed(&mut self, ctx: &mut PluginContext) -> usize {
        let mut count = 0;
        let mut i = 0;
        while i < self.plugins.len() {
            if !self.plugins[i].is_alive() && self.restart_plugin(i, ctx).is_ok() {
                count += 1;
            }
            i += 1;
        }
        count
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

    pub fn plugin_name(&self, idx: usize) -> Option<&str> {
        if idx < self.plugins.len() {
            Some(self.plugins[idx].name())
        } else {
            None
        }
    }

    pub fn is_ready(&self, idx: usize) -> bool {
        if idx < self.plugins.len() {
            self.plugins[idx].is_ready()
        } else {
            false
        }
    }

    pub fn has_dim_overlay(&self, idx: usize) -> bool {
        if idx < self.plugins.len() {
            self.plugins[idx].has_dim_overlay()
        } else {
            false
        }
    }

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
    /// Called once per frame (throttled to every 30 frames). Returns true if items changed.
    pub fn poll_registry_installed(&mut self) -> bool {
        self.registry_poll_skip = self.registry_poll_skip.saturating_sub(1);
        if self.registry_poll_skip > 0 {
            return false;
        }
        self.registry_poll_skip = 30;
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

    /// Re-read `registry.toml` and rebuild `dynamic_items` and `plugin_capabilities`.
    /// Returns true if items changed.
    pub fn read_registry_installed(&mut self) -> bool {
        self.registry_poll_skip = 0;
        self.invalidate_carousel_cache();
        let path = self.data_dir.join("registry.toml");
        let cfg = RegistryConfig::load(&path);
        let old = std::mem::take(&mut self.dynamic_items);
        self.plugin_capabilities.clear();

        let old_registry_ids = std::mem::take(&mut self.registry_plugin_ids);

        if let Some(ref cfg) = cfg {
            for installed in &cfg.plugins {
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
                self.registry_plugin_ids.insert(id.clone());
                if !installed.enabled {
                    continue;
                }
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
                self.plugin_capabilities
                    .insert(id.clone(), installed.capabilities.clone());
                if !self
                    .dynamic_items
                    .iter()
                    .any(|(_, existing_id, _)| *existing_id == id)
                {
                    self.dynamic_items
                        .push(("Plugins".into(), id.clone(), name));
                }
            }
            self.reap_unregistered(&old_registry_ids);
        }

        self.dynamic_items != old
    }

    /// Shut down and remove any loaded out-of-process plugins that were
    /// previously installed via the registry but are no longer present in the
    /// new config (deleted or disabled).  `old_ids` is the set of registry
    /// plugin ids from the *previous* read.
    fn reap_unregistered(&mut self, old_ids: &HashSet<String>) {
        let current: HashSet<&str> = self
            .dynamic_items
            .iter()
            .map(|(_, id, _)| id.as_str())
            .collect();
        let mut i = self.plugins.len();
        while i > 0 {
            i -= 1;
            let id = self.plugins[i].id();
            if self.plugins[i].binary_path().is_some()
                && old_ids.contains(id)
                && !current.contains(id)
            {
                self.plugins[i].on_blur();
                self.plugins[i].shutdown();
                self.plugins.remove(i);
                self.mtimes.remove(i);
            }
        }
        self.refresh_commands();
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}

/// An item in the home-screen carousel.
/// Represents a plugin that is either already loaded or available in the registry.
#[derive(Debug, Clone)]
pub struct CarouselItem {
    pub id: String,
    pub name: String,
}

impl PluginManager {
    fn invalidate_carousel_cache(&mut self) {
        self.carousel_cache = None;
    }

    /// Return the cached carousel items, rebuilding if necessary.
    pub fn carousel_items(&mut self) -> &[CarouselItem] {
        if self.carousel_cache.is_none() {
            self.carousel_cache = Some(self.build_carousel_items());
        }
        self.carousel_cache.as_deref().unwrap()
    }

    /// Build a unified carousel list from loaded plugins + enabled registry items.
    ///
    /// Order:
    /// 1. All currently loaded plugins (in registration order).
    /// 2. Enabled registry plugins whose id is not already loaded (deduplicated).
    fn build_carousel_items(&self) -> Vec<CarouselItem> {
        let mut items: Vec<CarouselItem> = self
            .plugins
            .iter()
            .map(|p| CarouselItem {
                id: p.id().to_string(),
                name: p.name().to_string(),
            })
            .collect();

        // Append registry items that aren't already loaded.
        let loaded_ids: HashSet<&str> = self.plugins.iter().map(|p| p.id()).collect();
        for (_, id, name) in &self.dynamic_items {
            if !loaded_ids.contains(id.as_str()) {
                items.push(CarouselItem {
                    id: id.clone(),
                    name: name.clone(),
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

/// Security: ensure `binary_path` resolves to a known-good directory so that
/// a tampered `registry.toml` cannot point to an arbitrary system binary.
///
/// Allowed directories:
/// - `<data_dir>` (covers `data_dir/plugins/` for registry-installed plugins)
/// - The santui executable directory (covers built-in plugins shipped alongside
///   `santui.exe`, including the `target/debug/` layout in dev mode).
fn validate_binary_path(
    binary_path: &Path,
    data_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let canonical = binary_path
        .canonicalize()
        .map_err(|_| format!("binary path does not exist: {}", binary_path.display()))?;
    if !canonical.is_file() {
        return Err(format!("binary path is not a file: {}", binary_path.display()).into());
    }

    let data_dir_canon = data_dir
        .canonicalize()
        .unwrap_or_else(|_| data_dir.to_path_buf());
    if canonical.starts_with(&data_dir_canon) {
        return Ok(());
    }

    if let Some(exe_dir) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .and_then(|d| d.canonicalize().ok())
    {
        if canonical.starts_with(&exe_dir) {
            return Ok(());
        }
    }

    Err(format!(
        "binary path {} is not within the santui data directory or executable directory",
        binary_path.display()
    )
    .into())
}
