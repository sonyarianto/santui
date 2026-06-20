use std::path::PathBuf;
use std::time::SystemTime;

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
        let path = dir.join("config.toml");
        if !path.exists() {
            return Config::default();
        }
        match std::fs::read_to_string(&path) {
            Ok(content) => match toml::from_str(&content) {
                Ok(cfg) => cfg,
                Err(e) => {
                    eprintln!("[santui] Failed to parse config.toml: {e}");
                    Config::default()
                }
            },
            Err(e) => {
                eprintln!("[santui] Failed to read config.toml: {e}");
                Config::default()
            }
        }
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
}

impl ConfigManager {
    /// Create a new manager, immediately loading the config from `dir`.
    pub fn new(dir: PathBuf) -> Self {
        let last_modified = dir
            .join("config.toml")
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok());
        let config = Config::load_from(&dir);
        ConfigManager {
            dir,
            config,
            last_modified,
            dirty: false,
        }
    }

    /// Re-read config from disk.  Call this once per frame.
    pub fn poll(&mut self) {
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
        let new_config = Config::load_from(&self.dir);
        self.config = new_config;
        self.dirty = true;
    }

    /// Acknowledge the dirty flag (call after applying changes).
    pub fn ack(&mut self) {
        self.dirty = false;
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Convenience: update the `theme` field and immediately persist.
    /// Syncs `last_modified` so the next `poll()` won't re-detect our write.
    pub fn save_theme(&mut self, theme_name: &str) {
        self.config.theme = Some(theme_name.to_string());
        if let Err(e) = self.config.save_to(&self.dir) {
            eprintln!("[santui] Failed to save config: {e}");
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
