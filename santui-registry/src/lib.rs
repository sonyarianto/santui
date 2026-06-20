mod config;
mod download;

use config::RegistryConfig;
use download::download_plugin;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A plugin entry advertised in the registry manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    /// GitHub release asset download URL for the current platform.
    pub download_url: String,
    /// SHA-256 hex digest of the binary.
    pub sha256: String,
    /// File size in bytes.
    pub size: u64,
}

/// A plugin the user has installed (either enabled or disabled).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledPlugin {
    pub enabled: bool,
    pub version: String,
    pub path: PathBuf,
}

/// Top-level state: fetched manifest + local installed set.
pub struct Registry {
    /// Manifest fetched from GitHub Releases.
    pub available: Vec<PluginManifest>,
    /// Locally installed plugins (keyed by plugin id).
    pub installed: Vec<InstalledPlugin>,
    /// Whether we've already fetched the manifest this session.
    pub fetched: bool,
    /// Human-readable status for the UI.
    pub status: String,
    config_path: PathBuf,
    plugins_dir: PathBuf,
}

impl Registry {
    /// Create a new registry rooted at `base_dir` (e.g. `~/.santui`).
    pub fn new(base_dir: PathBuf) -> Self {
        let plugins_dir = base_dir.join("plugins");
        let config_path = base_dir.join("registry.toml");
        let installed = RegistryConfig::load(&config_path)
            .map(|cfg| cfg.plugins)
            .unwrap_or_default();
        Registry {
            available: Vec::new(),
            installed,
            fetched: false,
            status: String::new(),
            config_path,
            plugins_dir,
        }
    }

    /// Fetch the plugin manifest from GitHub Releases.
    /// Uses `SANTUI_REPO` env or defaults to `sony-ak/santui`.
    pub fn fetch_manifest(&mut self) -> Result<(), String> {
        let repo = std::env::var("SANTUI_REPO").unwrap_or_else(|_| "sony-ak/santui".into());
        // Use the GitHub Releases API to get the latest release's plugins.json asset.
        let api_url = format!("https://api.github.com/repos/{repo}/releases/latest");
        let resp = ureq::get(&api_url)
            .header("User-Agent", "santui")
            .call()
            .map_err(|e| format!("Failed to fetch release: {e}"))?;

        let body = resp
            .into_body()
            .read_to_string()
            .map_err(|e| format!("Failed to read response: {e}"))?;

        // The release JSON has an `assets` array. Find the one named `plugins.json`.
        let release: serde_json::Value =
            serde_json::from_str(&body).map_err(|e| format!("Invalid JSON: {e}"))?;

        let assets = release["assets"]
            .as_array()
            .ok_or_else(|| "No assets in release".to_string())?;

        let plugin_asset = assets
            .iter()
            .find(|a| a["name"].as_str() == Some("plugins.json"))
            .ok_or_else(|| "No plugins.json found in release assets".to_string())?;

        let download_url = plugin_asset["browser_download_url"]
            .as_str()
            .ok_or_else(|| "Missing download_url".to_string())?;

        // Download and parse the manifest.
        let manifest_resp = ureq::get(download_url)
            .header("User-Agent", "santui")
            .call()
            .map_err(|e| format!("Failed to fetch manifest: {e}"))?;

        let manifest_body = manifest_resp
            .into_body()
            .read_to_string()
            .map_err(|e| format!("Failed to read manifest: {e}"))?;

        self.available = serde_json::from_str(&manifest_body)
            .map_err(|e| format!("Invalid manifest JSON: {e}"))?;
        self.fetched = true;
        self.status = format!("{} plugin(s) available", self.available.len());
        Ok(())
    }

    /// Download and install a plugin from the manifest.
    pub fn install(&mut self, manifest: &PluginManifest) -> Result<(), String> {
        std::fs::create_dir_all(&self.plugins_dir)
            .map_err(|e| format!("Failed to create plugins dir: {e}"))?;

        let target_path = self.plugins_dir.join(plugin_filename(&manifest.id));
        download_plugin(&manifest.download_url, &manifest.sha256, &target_path)?;

        self.installed.push(InstalledPlugin {
            enabled: true,
            version: manifest.version.clone(),
            path: target_path,
        });
        self.save_config()?;
        Ok(())
    }

    /// Enable or disable an installed plugin by index.
    pub fn set_enabled(&mut self, idx: usize, enabled: bool) -> Result<(), String> {
        if let Some(p) = self.installed.get_mut(idx) {
            p.enabled = enabled;
            self.save_config()?;
        }
        Ok(())
    }

    fn save_config(&self) -> Result<(), String> {
        let cfg = RegistryConfig {
            plugins: self.installed.clone(),
        };
        cfg.save(&self.config_path)
    }
}

/// Return the filename for a plugin binary on the current platform.
fn plugin_filename(id: &str) -> String {
    if cfg!(target_os = "windows") {
        format!("{id}.exe")
    } else {
        id.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_filename_windows() {
        // We can't easily test platform-specific logic, but the function
        // should return something with .exe on Windows.
        let name = plugin_filename("test-plugin");
        if cfg!(target_os = "windows") {
            assert!(name.ends_with(".exe"));
        } else {
            assert!(!name.ends_with(".exe"));
        }
    }
}
