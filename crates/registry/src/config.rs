use crate::InstalledPlugin;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct RegistryConfig {
    pub plugins: Vec<InstalledPlugin>,
}

impl RegistryConfig {
    pub fn load(path: &Path) -> Option<Self> {
        let text = std::fs::read_to_string(path).ok()?;
        toml::from_str(&text).ok()
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let text = toml::to_string_pretty(self).map_err(|e| format!("TOML serialize: {e}"))?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("Create config dir: {e}"))?;
        }
        std::fs::write(path, &text).map_err(|e| format!("Write config: {e}"))?;
        Ok(())
    }
}
