use std::path::PathBuf;
use std::time::{Duration, SystemTime};

/// Top-level Santui configuration, deserialized from `config.toml`.
#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
pub struct Config {
    /// Default theme name (must match a built-in theme or a custom theme name).
    pub theme: Option<String>,
    /// Custom theme color overrides.
    pub custom_theme: Option<CustomThemeColors>,
    /// Key-binding overrides (reserved — schema defined for future use).
    #[serde(default)]
    pub keybindings: Option<KeyBindings>,
    /// Plugin-specific settings (reserved — schema defined for future use).
    #[serde(default)]
    pub plugins: Option<PluginConfig>,
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

/// Key-binding overrides (reserved for future use).
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct KeyBindings {}

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

/// Watches `config.toml` for changes via periodic timestamp polling.
///
/// Call [`ConfigManager::poll`] once per frame in the main loop.  When the
/// file has been modified externally `dirty` is set to `true` and the new
/// config is available via [`ConfigManager::config`].
#[derive(Debug)]
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
    /// Throttle: only poll the filesystem every N frames.
    poll_skip: u32,
}

impl ConfigManager {
    /// Create a new manager, immediately loading the config from `dir`.
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
        ConfigManager {
            dir,
            config,
            last_modified,
            dirty: false,
            error,
            tick_rate: Duration::from_millis(100),
            poll_skip: 0,
        }
    }

    /// Re-read config from disk.  Call this once per frame.
    pub fn poll(&mut self) {
        self.poll_skip = self.poll_skip.saturating_sub(1);
        if self.poll_skip > 0 {
            return;
        }
        self.poll_skip = 30;
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
    /// so the next `poll()` doesn't re-detect our own write.
    fn persist(&mut self) {
        if let Err(e) = self.config.save_to(&self.dir) {
            log::error!("[santui] Failed to save config: {e}");
            return;
        }
        self.last_modified = self
            .dir
            .join("config.toml")
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok());
    }
}
