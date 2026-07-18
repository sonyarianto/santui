use std::fmt;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, SystemTime};

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};

/// Top-level Santui configuration, deserialized from `config.toml`.
#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
pub struct Config {
    /// Default theme name (must match a built-in theme or a custom theme name).
    pub theme: Option<String>,
    /// Custom theme color overrides.
    pub custom_theme: Option<CustomThemeColors>,
    /// Key-binding overrides.
    #[serde(default)]
    pub keybindings: KeyBindings,
    /// Enable mouse capture (click, scroll) in the terminal.
    /// When disabled, text selection works without holding Shift.
    /// Toggle at runtime with Alt+M.
    #[serde(default = "default_mouse_capture")]
    pub mouse_capture: bool,
    /// Plugin-specific settings (reserved — schema defined for future use).
    #[serde(default)]
    pub plugins: Option<PluginConfig>,
    /// Optional santui-server connection for state sync.
    #[serde(default)]
    pub server: Option<ServerConfig>,
}

fn default_mouse_capture() -> bool {
    false
}

/// Connection settings for a remote [`santui-server`](https://github.com/sonyarianto/santui).
///
/// When configured, the TUI app pushes plugin data changes to the server for
/// cross-device sync and persistence.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ServerConfig {
    /// Base URL of the santui-server, e.g. `"http://localhost:9876"`.
    pub url: String,
}

/// Per-color-field overrides for a custom theme.
///
/// Each field is an optional hex colour string like `"#ff8800"` or `"ff8800"`.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct CustomThemeColors {
    pub name: Option<String>,
    pub accent: Option<String>,
    pub highlight: Option<String>,
    pub logo: Option<String>,
    pub text: Option<String>,
    pub text_muted: Option<String>,
    pub background: Option<String>,
    pub background_panel: Option<String>,
    pub background_overlay: Option<String>,
    pub border: Option<String>,
    pub success: Option<String>,
    pub error: Option<String>,
    pub inverted_text: Option<String>,
}

/// Key-binding overrides.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct KeyBindings {
    /// Key to open the command palette. Default: Ctrl+P.
    /// Format: "ctrl+p", "ctrl+space", "alt+p"
    #[serde(default = "KeyBindings::default_open_palette")]
    pub open_palette: String,

    /// Key to quit the application. Default: "q"
    #[serde(default = "KeyBindings::default_quit")]
    pub quit: String,

    /// Key to show the about screen. Default: "?"
    #[serde(default = "KeyBindings::default_about")]
    pub about: String,
}

impl Default for KeyBindings {
    fn default() -> Self {
        Self {
            open_palette: Self::default_open_palette(),
            quit: Self::default_quit(),
            about: Self::default_about(),
        }
    }
}

impl KeyBindings {
    fn default_open_palette() -> String {
        "ctrl+p".into()
    }

    fn default_quit() -> String {
        "q".into()
    }

    fn default_about() -> String {
        "?".into()
    }

    /// Parse "ctrl+p" → (KeyCode::Char('p'), KeyModifiers::CONTROL).
    pub fn parse_key(
        s: &str,
    ) -> Option<(crossterm::event::KeyCode, crossterm::event::KeyModifiers)> {
        let s = s.to_lowercase();
        let parts: Vec<&str> = s.split('+').collect();
        let key_str = *parts.last()?;
        let mut mods = crossterm::event::KeyModifiers::NONE;
        for part in &parts[..parts.len().saturating_sub(1)] {
            match *part {
                "ctrl" => mods |= crossterm::event::KeyModifiers::CONTROL,
                "alt" => mods |= crossterm::event::KeyModifiers::ALT,
                "shift" => mods |= crossterm::event::KeyModifiers::SHIFT,
                _ => return None,
            }
        }
        let code = match key_str {
            "space" => crossterm::event::KeyCode::Char(' '),
            s if s.len() == 1 => crossterm::event::KeyCode::Char(s.chars().next()?),
            _ => return None,
        };
        Some((code, mods))
    }
}

/// Plugin-specific configuration (reserved for future use).
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct PluginConfig {}

impl Config {
    /// Load `config.toml` from `dir` or return a default config if the file
    /// doesn't exist.
    pub fn load_from(dir: &std::path::Path) -> Self {
        Self::try_load_from(dir).unwrap_or_else(|_| Config::default())
    }

    /// Like `load_from`, but returns an error message instead of silently
    /// falling back to defaults.
    pub fn try_load_from(dir: &std::path::Path) -> Result<Self, String> {
        let path = dir.join("config.toml");
        if !path.exists() {
            return Err("config.toml not found".into());
        }
        let content = std::fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read config.toml: {e}"))?;
        toml::from_str(&content).map_err(|e| format!("Failed to parse config.toml: {e}"))
    }

    /// Write the config to `dir/config.toml`.
    pub fn save_to(&self, dir: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
        let path = dir.join("config.toml");
        let content = toml::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }
}

/// Watches `config.toml` for changes via filesystem notifications, with
/// periodic mtime polling as a fallback.
///
/// Call [`ConfigManager::poll`] once per frame in the main loop.  When the
/// file has been modified externally `dirty` is set to `true` and the new
/// config is available via [`ConfigManager::config`].
pub struct ConfigManager {
    dir: PathBuf,
    config: Config,
    last_modified: Option<SystemTime>,
    /// Set to `true` by [`poll`](ConfigManager::poll) when the file changed.
    pub dirty: bool,
    /// Error message from the last load/parse attempt, cleared on ack.
    error: Option<String>,
    /// Main loop tick rate (how often the UI refreshes and polls for input).
    tick_rate: Duration,
    /// Throttle: only poll the filesystem every N frames (fallback path).
    poll_skip: u32,
    /// Filesystem watcher (kept alive for the lifetime of the manager).
    _watcher: Option<RecommendedWatcher>,
    /// Channel receiver for filesystem events.
    event_rx: Option<mpsc::Receiver<notify::Result<notify::Event>>>,
}

impl fmt::Debug for ConfigManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConfigManager")
            .field("dir", &self.dir)
            .field("config", &self.config)
            .field("last_modified", &self.last_modified)
            .field("dirty", &self.dirty)
            .field("error", &self.error)
            .field("tick_rate", &self.tick_rate)
            .field("poll_skip", &self.poll_skip)
            .finish()
    }
}

impl ConfigManager {
    /// Create a new manager, immediately loading the config from `dir` and
    /// attempting to set up a filesystem watcher.
    pub fn new(dir: PathBuf) -> Self {
        let last_modified = dir
            .join("config.toml")
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok());
        let (config, error) = match Config::try_load_from(&dir) {
            Ok(cfg) => (cfg, None),
            Err(e) => (Config::default(), Some(e)),
        };

        let (watcher, event_rx) = Self::try_create_watcher(&dir);

        ConfigManager {
            dir,
            config,
            last_modified,
            dirty: false,
            error,
            tick_rate: Duration::from_millis(100),
            poll_skip: 0,
            _watcher: watcher,
            event_rx,
        }
    }

    /// Create a new manager without a filesystem watcher (polling only).
    /// Behaviour is identical to [`new`](Self::new) but skips the watcher
    /// setup entirely.  Useful in tests where deterministic poll timing is
    /// required.
    #[cfg(test)]
    pub fn new_polling_only(dir: PathBuf) -> Self {
        let last_modified = dir
            .join("config.toml")
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok());
        let (config, error) = match Config::try_load_from(&dir) {
            Ok(cfg) => (cfg, None),
            Err(e) => (Config::default(), Some(e)),
        };
        ConfigManager {
            dir,
            config,
            last_modified,
            dirty: false,
            error,
            tick_rate: Duration::from_millis(100),
            poll_skip: 0,
            _watcher: None,
            event_rx: None,
        }
    }

    /// Try to set up a filesystem watcher on the config directory.
    /// Returns `None` on any failure (e.g. platform not supported).
    fn try_create_watcher(
        dir: &std::path::Path,
    ) -> (
        Option<RecommendedWatcher>,
        Option<mpsc::Receiver<notify::Result<notify::Event>>>,
    ) {
        let (tx, rx) = mpsc::channel();
        match RecommendedWatcher::new(tx, notify::Config::default()) {
            Ok(mut w) => match w.watch(dir, RecursiveMode::NonRecursive) {
                Ok(()) => {
                    log::info!("[santui] Filesystem watcher active on {:?}", dir);
                    (Some(w), Some(rx))
                }
                Err(e) => {
                    log::warn!("[santui] Failed to watch config dir: {e}; falling back to polling");
                    (None, None)
                }
            },
            Err(e) => {
                log::warn!(
                    "[santui] Failed to create filesystem watcher: {e}; falling back to polling"
                );
                (None, None)
            }
        }
    }

    /// Re-read config from disk.  Call this once per frame.
    ///
    /// When a filesystem watcher is active, events are drained immediately
    /// (no throttling).  The mtime-based polling fallback runs every 30 frames
    /// on platforms without a working watcher.
    pub fn poll(&mut self) {
        // Drain filesystem events immediately when watcher is available.
        if let Some(ref rx) = self.event_rx {
            let mut changed = false;
            while let Ok(Ok(event)) = rx.try_recv() {
                if Self::event_matches_config(&event) {
                    changed = true;
                }
            }
            if changed {
                return self.reload();
            }
        }

        // Polling fallback throttle.
        self.poll_skip = self.poll_skip.saturating_sub(1);
        if self.poll_skip > 0 {
            return;
        }
        self.poll_skip = 30;
        self.reload_if_modified();
    }

    /// Check whether a notify event applies to `config.toml`.
    fn event_matches_config(event: &notify::Event) -> bool {
        matches!(event.kind, EventKind::Create(_) | EventKind::Modify(_))
            && event
                .paths
                .iter()
                .any(|p| p.file_name().is_some_and(|n| n == "config.toml"))
    }

    /// Check if the file has been modified (by comparing mtime against
    /// `last_modified`) and reload if so.  This is the polling fallback.
    fn reload_if_modified(&mut self) {
        let path = self.dir.join("config.toml");
        let modified = match path.metadata().ok().and_then(|m| m.modified().ok()) {
            Some(t) => t,
            None => return,
        };
        let changed = match self.last_modified {
            Some(last) => modified != last,
            None => true,
        };
        if !changed {
            return;
        }
        self.last_modified = Some(modified);
        self.reload_inner();
    }

    /// Reload config from disk and update error/dirty state.
    fn reload(&mut self) {
        self.reload_if_modified();
    }

    fn reload_inner(&mut self) {
        match Config::try_load_from(&self.dir) {
            Ok(cfg) => {
                self.config = cfg;
                self.error = None;
                self.dirty = true;
            }
            Err(e) => {
                self.error = Some(e);
            }
        }
    }

    /// Acknowledge the dirty flag (call after applying changes).
    pub fn ack(&mut self) {
        self.dirty = false;
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Error message from the last failed config load/parse, if any.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Update the `theme` field and immediately persist.
    /// When selecting a built-in theme, custom overrides are cleared so they
    /// don't leak into the newly chosen theme.
    pub fn save_theme(&mut self, theme_name: &str) {
        self.config.theme = Some(theme_name.to_string());
        self.config.custom_theme = None;
        self.persist();
    }

    /// Set custom theme colour overrides in config and persist.
    pub fn save_custom_theme(&mut self, colors: CustomThemeColors) {
        self.config.custom_theme = Some(colors);
        self.persist();
    }

    pub fn tick_rate(&self) -> Duration {
        self.tick_rate
    }

    pub fn set_tick_rate(&mut self, duration: Duration) {
        self.tick_rate = duration;
    }

    /// Remove custom theme colour overrides from config and persist.
    pub fn clear_custom_theme(&mut self) {
        if self.config.custom_theme.is_some() {
            self.config.custom_theme = None;
            self.persist();
        }
    }

    /// Write the in-memory config to disk and sync the modification timestamp
    /// so reload calls don't re-detect our own write.
    ///
    /// Uses a temp-file + atomic rename to eliminate the TOCTOU window between
    /// writing and reading back the mtime (see AGENTS.md: no fragile solutions).
    fn persist(&mut self) {
        let path = self.dir.join("config.toml");
        let tmp_path = self.dir.join("config.toml.tmp");

        let content = match toml::to_string_pretty(&self.config) {
            Ok(c) => c,
            Err(e) => {
                log::error!("[santui] Failed to serialize config: {e}");
                return;
            }
        };

        if let Err(e) = std::fs::write(&tmp_path, &content) {
            log::error!("[santui] Failed to write config: {e}");
            let _ = std::fs::remove_file(&tmp_path);
            return;
        }

        // Capture mtime before the atomic rename — no window for an external
        // writer to slip in between our write and our metadata read.
        self.last_modified = tmp_path.metadata().ok().and_then(|m| m.modified().ok());

        if let Err(e) = std::fs::rename(&tmp_path, &path) {
            log::error!("[santui] Failed to rename config: {e}");
            let _ = std::fs::remove_file(&tmp_path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyModifiers};
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new() -> Self {
            let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
            let mut p = std::env::temp_dir();
            p.push(format!("santui_cfg_test_{id}"));
            let _ = std::fs::remove_dir_all(&p);
            std::fs::create_dir_all(&p).unwrap();
            TempDir { path: p }
        }

        fn path(&self) -> &std::path::Path {
            &self.path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn config_default_all_none() {
        let cfg = Config::default();
        assert!(cfg.theme.is_none());
        assert!(cfg.custom_theme.is_none());
        assert_eq!(cfg.keybindings.open_palette, "ctrl+p");
        assert_eq!(cfg.keybindings.quit, "q");
        assert_eq!(cfg.keybindings.about, "?");
        assert!(cfg.plugins.is_none());
        assert!(cfg.server.is_none());
    }

    #[test]
    fn try_load_from_missing_dir_returns_err() {
        let tmp = TempDir::new();
        let result = Config::try_load_from(tmp.path());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn try_load_from_invalid_toml_returns_err() {
        let tmp = TempDir::new();
        std::fs::write(tmp.path().join("config.toml"), "not toml {{{").unwrap();
        let result = Config::try_load_from(tmp.path());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("parse"));
    }

    #[test]
    fn load_from_missing_dir_returns_default() {
        let tmp = TempDir::new();
        let cfg = Config::load_from(tmp.path());
        assert!(cfg.theme.is_none());
    }

    #[test]
    fn save_to_and_load_from_roundtrip() {
        let tmp = TempDir::new();
        let cfg = Config {
            mouse_capture: false,
            theme: Some("Nord".into()),
            custom_theme: Some(CustomThemeColors {
                name: None,
                accent: Some("#ff8800".into()),
                highlight: None,
                logo: None,
                text: None,
                text_muted: None,
                background: None,
                background_panel: None,
                background_overlay: None,
                border: None,
                success: None,
                error: None,
                inverted_text: None,
            }),
            keybindings: KeyBindings::default(),
            plugins: None,
            server: None,
        };
        cfg.save_to(tmp.path()).unwrap();
        let loaded = Config::load_from(tmp.path());
        assert_eq!(loaded.theme.as_deref(), Some("Nord"));
    }

    #[test]
    fn config_manager_new_sets_last_modified() {
        let tmp = TempDir::new();
        let p = tmp.path().join("config.toml");
        std::fs::write(&p, r#"theme = "Nord""#).unwrap();
        let mgr = ConfigManager::new(tmp.path().to_path_buf());
        assert!(mgr.last_modified.is_some());
        assert!(!mgr.dirty);
    }

    #[test]
    fn config_manager_error_on_missing_file() {
        let tmp = TempDir::new();
        let mgr = ConfigManager::new(tmp.path().to_path_buf());
        assert!(mgr.error().is_some());
    }

    #[test]
    fn config_manager_error_cleared_on_successful_load() {
        let tmp = TempDir::new();
        let p = tmp.path().join("config.toml");
        std::fs::write(&p, r#"theme = "Nord""#).unwrap();
        let mgr = ConfigManager::new(tmp.path().to_path_buf());
        assert!(mgr.error().is_none());
    }

    #[test]
    fn config_manager_clear_noop_when_no_custom() {
        let tmp = TempDir::new();
        let mut mgr = ConfigManager::new(tmp.path().to_path_buf());
        mgr.clear_custom_theme();
        assert!(mgr.config().custom_theme.is_none());
    }

    #[test]
    fn config_manager_tick_rate_default_and_set() {
        let tmp = TempDir::new();
        let mut mgr = ConfigManager::new(tmp.path().to_path_buf());
        assert_eq!(mgr.tick_rate(), Duration::from_millis(100));
        mgr.set_tick_rate(Duration::from_millis(200));
        assert_eq!(mgr.tick_rate(), Duration::from_millis(200));
    }

    #[test]
    fn config_manager_poll_throttle_skips() {
        let tmp = TempDir::new();
        let p = tmp.path().join("config.toml");
        std::fs::write(&p, r#"theme = "Nord""#).unwrap();
        let mut mgr = ConfigManager::new_polling_only(tmp.path().to_path_buf());
        mgr.poll_skip = 5;
        mgr.ack();
        std::fs::write(&p, r#"theme = "Dracula""#).unwrap();
        mgr.poll();
        assert!(!mgr.dirty, "should skip due to poll_skip");
    }

    #[test]
    fn config_manager_poll_no_file_returns_early() {
        let tmp = TempDir::new();
        let mut mgr = ConfigManager::new_polling_only(tmp.path().to_path_buf());
        mgr.poll_skip = 0;
        // File doesn't exist, metadata returns None → poll returns early.
        mgr.poll();
        assert!(!mgr.dirty);
    }

    #[test]
    fn config_manager_watcher_detects_external_change() {
        let tmp = TempDir::new();
        let p = tmp.path().join("config.toml");
        std::fs::write(&p, r#"theme = "Nord""#).unwrap();
        let mut mgr = ConfigManager::new(tmp.path().to_path_buf());
        mgr.ack();
        assert!(!mgr.dirty);

        // Write a different config externally; the watcher should pick it up
        // on the next poll (regardless of poll_skip).
        std::fs::write(&p, r#"theme = "Dracula""#).unwrap();
        mgr.poll();
        assert!(mgr.dirty, "watcher should detect external change");
        assert_eq!(mgr.config().theme.as_deref(), Some("Dracula"));
    }

    #[test]
    fn config_manager_watcher_ignores_own_write() {
        let tmp = TempDir::new();
        let p = tmp.path().join("config.toml");
        std::fs::write(&p, r#"theme = "Nord""#).unwrap();
        let mut mgr = ConfigManager::new(tmp.path().to_path_buf());
        mgr.ack();
        assert!(!mgr.dirty);

        // persist() sets last_modified, so the watcher event should not
        // trigger a reload.
        mgr.save_theme("Dracula");
        assert!(!mgr.dirty, "own write should not set dirty");

        // Reading the file back should show the saved value.
        let loaded = Config::load_from(tmp.path());
        assert_eq!(loaded.theme.as_deref(), Some("Dracula"));
    }

    // ------------------------------------------------------------------
    // KeyBindings tests
    // ------------------------------------------------------------------

    #[test]
    fn keybindings_default_values() {
        let kb = KeyBindings::default();
        assert_eq!(kb.open_palette, "ctrl+p");
        assert_eq!(kb.quit, "q");
        assert_eq!(kb.about, "?");
    }

    #[test]
    fn parse_key_ctrl_p() {
        let (code, mods) = KeyBindings::parse_key("ctrl+p").unwrap();
        assert_eq!(code, KeyCode::Char('p'));
        assert!(mods.contains(KeyModifiers::CONTROL));
    }

    #[test]
    fn parse_key_plain_char() {
        let (code, mods) = KeyBindings::parse_key("q").unwrap();
        assert_eq!(code, KeyCode::Char('q'));
        assert!(mods.is_empty());
    }

    #[test]
    fn parse_key_question_mark() {
        let (code, mods) = KeyBindings::parse_key("?").unwrap();
        assert_eq!(code, KeyCode::Char('?'));
        assert!(mods.is_empty());
    }

    #[test]
    fn parse_key_ctrl_space() {
        let (code, mods) = KeyBindings::parse_key("ctrl+space").unwrap();
        assert_eq!(code, KeyCode::Char(' '));
        assert!(mods.contains(KeyModifiers::CONTROL));
    }

    #[test]
    fn parse_key_invalid_returns_none() {
        assert!(KeyBindings::parse_key("xyz+abc").is_none());
    }

    #[test]
    fn keybindings_deserialize_from_toml() {
        let toml_str = r#"
            [keybindings]
            open_palette = "alt+p"
            quit = "ctrl+q"
            about = "shift+/"
        "#;
        let cfg: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.keybindings.open_palette, "alt+p");
        assert_eq!(cfg.keybindings.quit, "ctrl+q");
        assert_eq!(cfg.keybindings.about, "shift+/");
    }

    #[test]
    fn keybindings_missing_from_toml_uses_defaults() {
        let toml_str = r#"
            theme = "Nord"
        "#;
        let cfg: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.keybindings.open_palette, "ctrl+p");
        assert_eq!(cfg.keybindings.quit, "q");
        assert_eq!(cfg.keybindings.about, "?");
    }
}
